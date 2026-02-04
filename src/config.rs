// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

use std::path::PathBuf;
use std::time::Duration;

/// API routing mode configuration
#[derive(Debug, Clone)]
pub enum ApiRoutingMode {
    /// Serve Admin API on a separate port
    SeparatePort { port: u16 },
    /// Serve Admin API at a path prefix on the mock port
    PathPrefix { prefix: String },
    /// Serve Admin API at a specific domain
    Domain { domain: String },
}

/// Server configuration (static, loaded at startup)
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Port for mock server
    pub port: u16,
    /// Bind address
    pub host: String,
    /// Directory containing Lua mock files
    pub mocks_dir: PathBuf,
    /// Directory for SQLite database
    pub data_dir: PathBuf,
    /// API routing configuration
    pub api_routing: ApiRoutingMode,
    /// Days to retain request history
    pub retention_days: u32,
    /// Maximum request body size in bytes
    pub max_body_size: usize,
    /// Lua script execution timeout
    pub script_timeout: Duration,
    /// Idle domain flush timeout (0 = disabled)
    pub idle_timeout: Duration,
    /// Memory limit per Lua state in MB
    pub lua_memory_mb: usize,
    /// SQLite page cache size in MB
    pub db_cache_mb: u32,
    /// Enable hot reload of Lua files
    pub watch_enabled: bool,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 3000,
            host: "127.0.0.1".to_string(),
            mocks_dir: PathBuf::from("./.mockserver/mocks"),
            data_dir: PathBuf::from("./.mockserver/data"),
            api_routing: ApiRoutingMode::SeparatePort { port: 3001 },
            retention_days: 7,
            max_body_size: 10 * 1024 * 1024, // 10MB
            script_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(30 * 60), // 30 minutes
            lua_memory_mb: 64,
            db_cache_mb: 64,
            watch_enabled: true,
        }
    }
}

impl ServerConfig {
    /// Get the database file path
    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("mockserver.db")
    }
}
