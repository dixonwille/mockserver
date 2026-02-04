// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Lua script manager with per-domain runtime pooling
//!
//! Manages pools of Lua states for each domain with sandboxing and isolation.
//! Each domain has its own pool of runtimes that can handle concurrent requests.

use crate::error::{Error, Result};
use crate::lua::modules::{DomainState, register_modules};
use crate::lua::sandbox::{configure_package_path, sandbox_lua};
use dashmap::DashMap;
use mlua::{Function, Lua, RegistryKey, Table, Value};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};

/// Configuration for Lua runtime pools
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum runtimes per domain (default: num CPUs)
    pub max_per_domain: usize,
    /// Time before shrinking oversized idle pools
    pub shrink_interval: Duration,
    /// Idle time before shrinking pool to min
    pub idle_timeout: Duration,
    /// Idle time before unloading entire domain
    pub domain_unload_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_per_domain: std::thread::available_parallelism()
                .map(|p| p.get())
                .unwrap_or(4),
            shrink_interval: Duration::from_secs(60),
            idle_timeout: Duration::from_secs(30),
            domain_unload_timeout: Duration::from_secs(300),
        }
    }
}

/// A single Lua runtime instance
struct LuaRuntime {
    /// The Lua state
    lua: Lua,
    /// Registry key for the handle function (survives GC)
    handle_key: RegistryKey,
}

/// Pool of Lua runtimes for a single domain
pub struct DomainPool {
    /// Available (idle) Lua runtimes
    available: Mutex<Vec<LuaRuntime>>,
    /// Count of runtimes currently in use
    in_use: AtomicUsize,
    /// Maximum pool size
    max_size: usize,
    /// Shared state across all runtimes (already thread-safe)
    state: DomainState,
    /// Domain name
    domain: String,
    /// Domain directory path
    domain_dir: PathBuf,
    /// Cached init.lua content for creating new runtimes
    init_content: String,
    /// Memory limit per Lua state
    memory_limit: usize,
    /// Last access time for idle cleanup
    last_access: Mutex<Instant>,
}

impl DomainPool {
    /// Create a new domain pool
    fn new(
        domain: String,
        domain_dir: PathBuf,
        init_content: String,
        max_size: usize,
        memory_limit: usize,
    ) -> Self {
        Self {
            available: Mutex::new(Vec::new()),
            in_use: AtomicUsize::new(0),
            max_size,
            state: Arc::new(RwLock::new(HashMap::new())),
            domain,
            domain_dir,
            init_content,
            memory_limit,
            last_access: Mutex::new(Instant::now()),
        }
    }

    /// Update last access time
    fn touch(&self) {
        *self.last_access.lock().unwrap() = Instant::now();
    }

    /// Get last access time
    fn last_access(&self) -> Instant {
        *self.last_access.lock().unwrap()
    }

    /// Create a new Lua runtime for this domain
    fn create_runtime(&self) -> Result<LuaRuntime> {
        let lua = Lua::new();

        // Set memory limit
        if self.memory_limit > 0 {
            lua.set_memory_limit(self.memory_limit)?;
        }

        // Apply sandboxing
        sandbox_lua(&lua)?;

        // Configure package.path for domain isolation
        configure_package_path(&lua, &self.domain_dir)?;

        // Register host modules with shared state
        register_modules(&lua, &self.domain, &self.domain_dir, self.state.clone())?;

        // Load and execute init.lua
        lua.load(&self.init_content)
            .set_name(format!("{}/init.lua", self.domain))
            .exec()?;

        // Get handle() function and store in registry
        let handle_fn: Function = lua
            .globals()
            .get::<Function>("handle")
            .map_err(|_| Error::MissingHandleFunction(self.domain.clone()))?;
        let handle_key = lua.create_registry_value(handle_fn)?;

        Ok(LuaRuntime { lua, handle_key })
    }

    /// Acquire a Lua runtime from the pool, creating one if needed
    fn acquire(&self) -> Result<PoolGuard<'_>> {
        self.touch();

        // Try to get an existing runtime
        {
            let mut available = self.available.lock().unwrap();
            if let Some(runtime) = available.pop() {
                drop(available);
                self.in_use.fetch_add(1, Ordering::SeqCst);
                return Ok(PoolGuard {
                    runtime: Some(runtime),
                    pool: self,
                });
            }
        }

        // Check if we can create a new one
        let current = self.in_use.load(Ordering::SeqCst);
        if current >= self.max_size {
            return Err(Error::PoolExhausted(self.domain.clone()));
        }

        // Create new runtime
        self.in_use.fetch_add(1, Ordering::SeqCst);
        match self.create_runtime() {
            Ok(runtime) => Ok(PoolGuard {
                runtime: Some(runtime),
                pool: self,
            }),
            Err(e) => {
                self.in_use.fetch_sub(1, Ordering::SeqCst);
                Err(e)
            }
        }
    }

    /// Return a runtime to the pool
    fn release(&self, runtime: LuaRuntime) {
        self.in_use.fetch_sub(1, Ordering::SeqCst);
        self.available.lock().unwrap().push(runtime);
    }

    /// Shrink pool to target size, dropping excess idle runtimes
    pub fn shrink_to(&self, target: usize) -> usize {
        let mut available = self.available.lock().unwrap();
        let initial = available.len();
        while available.len() > target {
            available.pop();
        }
        initial - available.len()
    }

    /// Get current pool statistics
    pub fn stats(&self) -> PoolStats {
        let available = self.available.lock().unwrap();
        PoolStats {
            available: available.len(),
            in_use: self.in_use.load(Ordering::SeqCst),
            max_size: self.max_size,
        }
    }
}

/// RAII guard that returns runtime to pool on drop
struct PoolGuard<'a> {
    runtime: Option<LuaRuntime>,
    pool: &'a DomainPool,
}

impl<'a> PoolGuard<'a> {
    /// Get reference to the Lua state
    fn lua(&self) -> &Lua {
        &self.runtime.as_ref().unwrap().lua
    }

    /// Get reference to the handle function registry key
    fn handle_key(&self) -> &RegistryKey {
        &self.runtime.as_ref().unwrap().handle_key
    }
}

impl Drop for PoolGuard<'_> {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            self.pool.release(runtime);
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone)]
pub struct PoolStats {
    pub available: usize,
    pub in_use: usize,
    pub max_size: usize,
}

/// HTTP request data passed to Lua handlers
#[derive(Debug, Clone)]
pub struct LuaRequest {
    pub method: String,
    pub path: String,
    pub query: HashMap<String, String>,
    pub headers: HashMap<String, String>,
    pub body: String,
    pub domain: String,
}

/// HTTP response returned from Lua handlers
#[derive(Debug, Clone, Default)]
pub struct LuaResponse {
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub body: String,
}

/// Information about a domain directory
#[derive(Debug, Clone)]
pub struct DomainEntry {
    /// Domain name (directory name)
    pub name: String,
    /// Path to the domain directory
    pub path: PathBuf,
    /// Path to init.lua (may not exist)
    pub init_path: PathBuf,
    /// Whether init.lua exists
    pub has_init: bool,
}

/// List all domain directories in the mocks folder.
/// Skips hidden directories (starting with '.') and non-directories.
pub fn list_domains(mocks_dir: &Path) -> io::Result<Vec<DomainEntry>> {
    let mut domains = Vec::new();

    for entry in std::fs::read_dir(mocks_dir)? {
        let entry = entry?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };

        // Skip hidden directories (e.g., .git) and _types (type definitions)
        if name.starts_with('.') || name == "_types" {
            continue;
        }

        let init_path = path.join("init.lua");
        let has_init = init_path.exists();

        domains.push(DomainEntry {
            name,
            path,
            init_path,
            has_init,
        });
    }

    domains.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(domains)
}

/// Manager for loading and executing Lua domain scripts with pooling
pub struct ScriptManager {
    /// Root directory containing domain folders
    mocks_dir: PathBuf,
    /// Per-domain pools (lock-free concurrent access)
    pools: DashMap<String, Arc<DomainPool>>,
    /// Memory limit per Lua state (in bytes)
    memory_limit: usize,
    /// Script execution timeout
    timeout: Duration,
    /// Pool configuration
    pool_config: PoolConfig,
}

impl ScriptManager {
    /// Create a new script manager
    pub fn new(mocks_dir: PathBuf, memory_limit_mb: usize, timeout: Duration) -> Self {
        Self {
            mocks_dir,
            pools: DashMap::new(),
            memory_limit: memory_limit_mb * 1024 * 1024,
            timeout,
            pool_config: PoolConfig::default(),
        }
    }

    /// Create a new script manager with custom pool configuration
    pub fn with_pool_config(
        mocks_dir: PathBuf,
        memory_limit_mb: usize,
        timeout: Duration,
        pool_config: PoolConfig,
    ) -> Self {
        Self {
            mocks_dir,
            pools: DashMap::new(),
            memory_limit: memory_limit_mb * 1024 * 1024,
            timeout,
            pool_config,
        }
    }

    /// Load all domains from the mocks directory (creates pools but not runtimes)
    pub async fn load_all(&self) -> Result<()> {
        for entry in list_domains(&self.mocks_dir)? {
            if entry.has_init {
                match self.load_domain(&entry.name).await {
                    Ok(_) => info!(domain = entry.name, "Loaded domain pool"),
                    Err(e) => warn!(domain = entry.name, error = %e, "Failed to load domain"),
                }
            }
        }

        Ok(())
    }

    /// Load or reload a specific domain (creates pool, not runtimes yet)
    pub async fn load_domain(&self, domain: &str) -> Result<()> {
        let domain_dir = self.mocks_dir.join(domain);
        let init_path = domain_dir.join("init.lua");

        if !init_path.exists() {
            return Err(Error::ScriptNotFound(domain.to_string()));
        }

        // Read init.lua content
        let init_content = std::fs::read_to_string(&init_path)?;

        // Validate by creating one runtime (ensures script is valid)
        let pool = DomainPool::new(
            domain.to_string(),
            domain_dir,
            init_content,
            self.pool_config.max_per_domain,
            self.memory_limit,
        );

        // Create one runtime to validate the script
        let runtime = pool.create_runtime()?;
        pool.available.lock().unwrap().push(runtime);

        self.pools.insert(domain.to_string(), Arc::new(pool));

        debug!(domain, "Domain pool created");
        Ok(())
    }

    /// Unload a specific domain
    pub async fn unload_domain(&self, domain: &str) {
        if self.pools.remove(domain).is_some() {
            info!(domain, "Domain unloaded");
        }
    }

    /// Execute a request against a domain's handler
    pub async fn execute(&self, request: LuaRequest) -> Result<LuaResponse> {
        let domain = &request.domain;

        // Get or create domain pool
        let pool = self.get_or_create_pool(domain).await?;

        // Acquire runtime from pool (only locks this domain's pool)
        let guard = pool.acquire()?;

        // Execute handler
        execute_handler_async(guard.lua(), guard.handle_key(), &request, self.timeout).await
    }

    /// Get existing pool or create new one (with _default fallback)
    async fn get_or_create_pool(&self, domain: &str) -> Result<Arc<DomainPool>> {
        // Fast path: pool exists
        if let Some(pool) = self.pools.get(domain) {
            return Ok(pool.clone());
        }

        // Try to load the domain
        if self.load_domain(domain).await.is_ok()
            && let Some(pool) = self.pools.get(domain)
        {
            return Ok(pool.clone());
        }

        // Fall back to _default
        if domain != "_default" {
            if let Some(pool) = self.pools.get("_default") {
                return Ok(pool.clone());
            }

            // Try to load _default
            if self.load_domain("_default").await.is_ok()
                && let Some(pool) = self.pools.get("_default")
            {
                return Ok(pool.clone());
            }
        }

        Err(Error::ScriptNotFound(domain.to_string()))
    }

    /// Check if a domain is loaded
    pub fn is_loaded(&self, domain: &str) -> bool {
        self.pools.contains_key(domain)
    }

    /// Get list of loaded domains
    pub fn loaded_domains(&self) -> Vec<String> {
        self.pools.iter().map(|r| r.key().clone()).collect()
    }

    /// Get pool statistics for a domain
    pub fn pool_stats(&self, domain: &str) -> Option<PoolStats> {
        self.pools.get(domain).map(|pool| pool.stats())
    }

    /// Flush domains that have been idle for longer than the specified duration
    pub fn flush_idle_domains(&self, idle_timeout: Duration) -> Vec<String> {
        let now = Instant::now();
        let mut flushed = Vec::new();

        // Collect domains to remove
        let to_remove: Vec<String> = {
            let mut candidates = Vec::new();
            for entry in self.pools.iter() {
                let domain = entry.key();
                // Never flush _default
                if domain == "_default" {
                    continue;
                }
                let pool = entry.value();
                let last_access = pool.last_access();
                if now.duration_since(last_access) > idle_timeout {
                    candidates.push(domain.clone());
                }
            }
            candidates
        };

        // Remove them
        for domain in to_remove {
            if self.pools.remove(&domain).is_some() {
                info!(domain, "Flushed idle domain");
                flushed.push(domain);
            }
        }

        flushed
    }

    /// Shrink all pools that have been idle
    pub fn shrink_idle_pools(&self) {
        let now = Instant::now();

        for entry in self.pools.iter() {
            let pool = entry.value();
            let last_access = pool.last_access();

            if now.duration_since(last_access) > self.pool_config.idle_timeout {
                let shrunk = pool.shrink_to(0);
                if shrunk > 0 {
                    debug!(domain = entry.key(), shrunk, "Shrunk idle pool");
                }
            }
        }
    }

    /// Get the path to a domain's init.lua
    pub fn domain_init_path(&self, domain: &str) -> PathBuf {
        self.mocks_dir.join(domain).join("init.lua")
    }

    /// Get the mocks directory
    pub fn mocks_dir(&self) -> &Path {
        &self.mocks_dir
    }

    /// Get the pool configuration
    pub fn pool_config(&self) -> &PoolConfig {
        &self.pool_config
    }
}

/// Execute the handle() function asynchronously (supports async Lua functions)
async fn execute_handler_async(
    lua: &Lua,
    handle_key: &RegistryKey,
    request: &LuaRequest,
    _timeout: Duration,
) -> Result<LuaResponse> {
    // Build request table
    let request_table = lua.create_table()?;
    request_table.set("method", request.method.as_str())?;
    request_table.set("path", request.path.as_str())?;
    request_table.set("body", request.body.as_str())?;
    request_table.set("domain", request.domain.as_str())?;

    // Query params
    let query_table = lua.create_table()?;
    for (k, v) in &request.query {
        query_table.set(k.as_str(), v.as_str())?;
    }
    request_table.set("query", query_table)?;

    // Headers (lowercase keys)
    let headers_table = lua.create_table()?;
    for (k, v) in &request.headers {
        headers_table.set(k.to_lowercase(), v.as_str())?;
    }
    request_table.set("headers", headers_table)?;

    // Get handle function from registry (survives GC)
    let handle: Function = lua.registry_value(handle_key)?;

    // Execute asynchronously to support async Lua functions (e.g., delay.sleep)
    let result: Value = handle.call_async(request_table).await?;

    parse_response(result)
}

/// Parse a Lua response table into LuaResponse
fn parse_response(value: Value) -> Result<LuaResponse> {
    let mut response = LuaResponse {
        status: 200,
        ..Default::default()
    };

    match value {
        Value::Table(t) => {
            // Status (optional, defaults to 200)
            if let Ok(status) = t.get::<u16>("status") {
                response.status = status;
            }

            // Body (optional)
            if let Ok(body) = t.get::<String>("body") {
                response.body = body;
            }

            // Headers (optional)
            if let Ok(headers) = t.get::<Table>("headers") {
                for (k, v) in headers.pairs::<String, String>().flatten() {
                    response.headers.insert(k, v);
                }
            }
        }
        Value::Nil => {
            // No response returned, use defaults
        }
        _ => {
            return Err(Error::Lua(mlua::Error::external(
                "handle() must return a table or nil",
            )));
        }
    }

    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_domain(temp_dir: &TempDir, domain: &str, script: &str) -> PathBuf {
        let domain_dir = temp_dir.path().join(domain);
        std::fs::create_dir_all(&domain_dir).unwrap();
        let init_path = domain_dir.join("init.lua");
        std::fs::write(&init_path, script).unwrap();
        domain_dir
    }

    #[tokio::test]
    async fn test_load_and_execute_domain() {
        let temp_dir = TempDir::new().unwrap();
        create_test_domain(
            &temp_dir,
            "test.example.com",
            r#"
            function handle(request)
                return {
                    status = 200,
                    headers = { ["Content-Type"] = "text/plain" },
                    body = "Hello, " .. request.method
                }
            end
            "#,
        );

        let manager =
            ScriptManager::new(temp_dir.path().to_path_buf(), 64, Duration::from_secs(30));

        manager.load_domain("test.example.com").await.unwrap();

        let request = LuaRequest {
            method: "GET".to_string(),
            path: "/test".to_string(),
            query: HashMap::new(),
            headers: HashMap::new(),
            body: String::new(),
            domain: "test.example.com".to_string(),
        };

        let response = manager.execute(request).await.unwrap();

        assert_eq!(response.status, 200);
        assert_eq!(response.body, "Hello, GET");
        assert_eq!(
            response.headers.get("Content-Type"),
            Some(&"text/plain".to_string())
        );
    }

    #[tokio::test]
    async fn test_missing_handle_function() {
        let temp_dir = TempDir::new().unwrap();
        create_test_domain(
            &temp_dir,
            "bad.example.com",
            r#"
            -- No handle function defined
            local x = 1
            "#,
        );

        let manager =
            ScriptManager::new(temp_dir.path().to_path_buf(), 64, Duration::from_secs(30));

        let result = manager.load_domain("bad.example.com").await;
        assert!(matches!(result, Err(Error::MissingHandleFunction(_))));
    }

    #[tokio::test]
    async fn test_flush_idle_domains() {
        let temp_dir = TempDir::new().unwrap();
        create_test_domain(
            &temp_dir,
            "_default",
            "function handle(r) return {status=200} end",
        );
        create_test_domain(
            &temp_dir,
            "idle.example.com",
            "function handle(r) return {status=200} end",
        );

        let manager =
            ScriptManager::new(temp_dir.path().to_path_buf(), 64, Duration::from_secs(30));

        manager.load_domain("_default").await.unwrap();
        manager.load_domain("idle.example.com").await.unwrap();

        // With zero timeout, all non-default domains should be flushed
        let flushed = manager.flush_idle_domains(Duration::ZERO);

        assert!(flushed.contains(&"idle.example.com".to_string()));
        assert!(!flushed.contains(&"_default".to_string()));
        assert!(manager.is_loaded("_default"));
        assert!(!manager.is_loaded("idle.example.com"));
    }

    #[tokio::test]
    async fn test_concurrent_requests_same_domain() {
        let temp_dir = TempDir::new().unwrap();
        create_test_domain(
            &temp_dir,
            "concurrent.example.com",
            r#"
            local state = require("state")
            function handle(request)
                local count = state.get("count") or 0
                count = count + 1
                state.set("count", count)
                return {
                    status = 200,
                    body = tostring(count)
                }
            end
            "#,
        );

        let manager = Arc::new(ScriptManager::new(
            temp_dir.path().to_path_buf(),
            64,
            Duration::from_secs(30),
        ));

        manager.load_domain("concurrent.example.com").await.unwrap();

        // Spawn multiple concurrent requests
        let mut handles = Vec::new();
        for _ in 0..10 {
            let mgr = Arc::clone(&manager);
            handles.push(tokio::spawn(async move {
                let request = LuaRequest {
                    method: "GET".to_string(),
                    path: "/test".to_string(),
                    query: HashMap::new(),
                    headers: HashMap::new(),
                    body: String::new(),
                    domain: "concurrent.example.com".to_string(),
                };
                mgr.execute(request).await.unwrap()
            }));
        }

        // Wait for all to complete
        let results: Vec<_> = futures::future::join_all(handles)
            .await
            .into_iter()
            .map(|r| r.unwrap())
            .collect();

        // All requests should succeed
        assert_eq!(results.len(), 10);
        for response in &results {
            assert_eq!(response.status, 200);
        }

        // Final count should be 10 (state is shared)
        let final_request = LuaRequest {
            method: "GET".to_string(),
            path: "/test".to_string(),
            query: HashMap::new(),
            headers: HashMap::new(),
            body: String::new(),
            domain: "concurrent.example.com".to_string(),
        };
        let final_response = manager.execute(final_request).await.unwrap();
        assert_eq!(final_response.body, "11"); // 10 + 1 more
    }

    #[tokio::test]
    async fn test_pool_stats() {
        let temp_dir = TempDir::new().unwrap();
        create_test_domain(
            &temp_dir,
            "stats.example.com",
            "function handle(r) return {status=200} end",
        );

        let manager =
            ScriptManager::new(temp_dir.path().to_path_buf(), 64, Duration::from_secs(30));

        manager.load_domain("stats.example.com").await.unwrap();

        let stats = manager.pool_stats("stats.example.com").unwrap();
        assert_eq!(stats.available, 1); // One pre-created during load
        assert_eq!(stats.in_use, 0);
    }
}
