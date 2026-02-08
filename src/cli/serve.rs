// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! The `serve` command - starts the mock server.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Args;
use tokio::net::TcpListener;
use tracing::{info, warn};

use crate::ServerConfig;
use crate::api::build_api_prefixed_router;
use crate::api::build_api_router;
use crate::config::ApiRoutingMode;
use crate::db::init_database;
use crate::lua::ScriptManager;
use crate::server::{AppState, build_domain_dispatch_router, build_mock_router};
use crate::watcher::{start_idle_flusher, start_watcher};

#[derive(Args)]
pub struct ServeArgs {
    /// Port for mock server
    #[arg(short, long, default_value = "3000", env = "MOCKSERVER_PORT")]
    port: u16,

    /// Directory containing Lua mock files
    #[arg(
        short,
        long,
        default_value = "./.mockserver/mocks",
        env = "MOCKSERVER_DIR"
    )]
    dir: PathBuf,

    /// Directory for SQLite database
    #[arg(
        long,
        default_value = "./.mockserver/data",
        env = "MOCKSERVER_DATA_DIR"
    )]
    data_dir: PathBuf,

    /// Serve Admin API on separate port
    #[arg(long, default_value = "3001", env = "MOCKSERVER_API_PORT")]
    api_port: u16,

    /// Serve Admin API at path prefix (disables --api-port)
    #[arg(long, env = "MOCKSERVER_API_PREFIX", conflicts_with = "api_domain")]
    api_prefix: Option<String>,

    /// Serve Admin API at specific domain (disables --api-port)
    #[arg(long, env = "MOCKSERVER_API_DOMAIN", conflicts_with = "api_prefix")]
    api_domain: Option<String>,

    /// Bind address
    #[arg(long, default_value = "127.0.0.1", env = "MOCKSERVER_HOST")]
    host: String,

    /// Days to retain request history
    #[arg(long, default_value = "7", env = "MOCKSERVER_RETENTION")]
    retention: u32,

    /// Maximum request body size in bytes
    #[arg(long, default_value = "10485760", env = "MOCKSERVER_MAX_BODY")]
    max_body: usize,

    /// Lua script execution timeout in seconds
    #[arg(long, default_value = "30", env = "MOCKSERVER_SCRIPT_TIMEOUT")]
    script_timeout: u64,

    /// Flush idle domain Lua states after N minutes (0 to disable)
    #[arg(long, default_value = "30", env = "MOCKSERVER_IDLE_TIMEOUT")]
    idle_timeout: u64,

    /// Memory limit per Lua domain state in MB
    #[arg(long, default_value = "64", env = "MOCKSERVER_LUA_MEMORY")]
    lua_memory: usize,

    /// SQLite page cache size in MB
    #[arg(long, default_value = "64", env = "MOCKSERVER_DB_CACHE")]
    db_cache: u32,

    /// Disable hot-reload of Lua files
    #[arg(long)]
    no_watch: bool,
}

impl From<ServeArgs> for ServerConfig {
    fn from(args: ServeArgs) -> Self {
        let api_routing = if let Some(domain) = args.api_domain {
            let domain = domain.split(':').next().unwrap_or(&domain).to_lowercase();
            ApiRoutingMode::Domain { domain }
        } else if let Some(prefix) = args.api_prefix {
            ApiRoutingMode::PathPrefix { prefix }
        } else {
            ApiRoutingMode::SeparatePort {
                port: args.api_port,
            }
        };

        ServerConfig {
            port: args.port,
            host: args.host,
            mocks_dir: args.dir,
            data_dir: args.data_dir,
            api_routing,
            retention_days: args.retention,
            max_body_size: args.max_body,
            script_timeout: std::time::Duration::from_secs(args.script_timeout),
            idle_timeout: std::time::Duration::from_secs(args.idle_timeout * 60),
            lua_memory_mb: args.lua_memory,
            db_cache_mb: args.db_cache,
            watch_enabled: !args.no_watch,
        }
    }
}

impl ServeArgs {
    pub async fn run(self) -> anyhow::Result<()> {
        let config = ServerConfig::from(self);

        info!("Starting mockserver v{}...", env!("CARGO_PKG_VERSION"));

        // 1. Ensure directories exist
        std::fs::create_dir_all(&config.mocks_dir)?;
        std::fs::create_dir_all(&config.data_dir)?;

        // Check if mocks directory is initialized
        if !config.mocks_dir.join("_default").exists() {
            anyhow::bail!(
                "Mocks directory not initialized. Run: mockserver init {:?}",
                config.mocks_dir
            );
        }

        // 2. Initialize database
        let db_path = config.db_path();
        info!("Database: {}", db_path.display());
        let db = init_database(
            db_path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Invalid database path"))?,
            config.db_cache_mb,
        )
        .await?;

        // 3. Initialize script manager
        let scripts = Arc::new(ScriptManager::new(
            config.mocks_dir.clone(),
            config.lua_memory_mb,
            config.script_timeout,
        ));

        // Load all domains at startup
        scripts.load_all().await?;
        let loaded = scripts.loaded_domains();
        info!("Loaded {} domain(s): {:?}", loaded.len(), loaded);

        if let ApiRoutingMode::Domain { domain } = &config.api_routing
            && loaded.iter().any(|d| d.eq_ignore_ascii_case(domain))
        {
            warn!(
                "API domain '{}' matches a loaded mock domain — mock scripts for this domain will be unreachable",
                domain
            );
        }

        // 4. Start file watcher (if enabled)
        let _watcher_handle = if config.watch_enabled {
            info!("Hot reload enabled");
            Some(start_watcher(config.mocks_dir.clone(), scripts.clone()).await?)
        } else {
            info!("Hot reload disabled");
            None
        };

        // 5. Start idle flusher (if enabled)
        let _flusher_handle = if config.idle_timeout.as_secs() > 0 {
            info!(
                "Idle domain flushing enabled (timeout: {} minutes)",
                config.idle_timeout.as_secs() / 60
            );
            Some(start_idle_flusher(scripts.clone(), config.idle_timeout))
        } else {
            info!("Idle domain flushing disabled");
            None
        };

        // 6. Create application state
        let config = Arc::new(config);
        let db = Arc::new(db);
        let state = AppState::from_arc(db, scripts.clone(), config.clone());

        // 7. Build routers and start servers based on API routing mode
        match &config.api_routing {
            ApiRoutingMode::SeparatePort { port } => {
                // Mock server on main port
                let mock_router = build_mock_router(state.clone());
                let mock_addr = format!("{}:{}", config.host, config.port);
                let mock_listener = TcpListener::bind(&mock_addr).await?;
                info!("Mock server listening on http://{}", mock_addr);

                // API server on separate port
                let api_router = build_api_router(state);
                let api_addr = format!("{}:{}", config.host, port);
                let api_listener = TcpListener::bind(&api_addr).await?;
                info!("Admin API listening on http://{}/api", api_addr);

                // Run both servers concurrently
                tokio::select! {
                    result = axum::serve(mock_listener, mock_router) => {
                        result?;
                    }
                    result = axum::serve(api_listener, api_router) => {
                        result?;
                    }
                }
            }
            ApiRoutingMode::PathPrefix { prefix } => {
                // Combined router with API under prefix
                let api_router = build_api_prefixed_router(state.clone(), prefix);
                let mock_router = build_mock_router(state);
                let combined = api_router.merge(mock_router);

                let addr = format!("{}:{}", config.host, config.port);
                let listener = TcpListener::bind(&addr).await?;
                info!("Server listening on http://{}", addr);
                info!("Admin API at http://{}{}", addr, prefix);

                axum::serve(listener, combined).await?;
            }
            ApiRoutingMode::Domain { domain } => {
                let api_router = build_api_router(state.clone());
                let mock_router = build_mock_router(state);
                let app = build_domain_dispatch_router(api_router, mock_router, domain);

                let addr = format!("{}:{}", config.host, config.port);
                let listener = TcpListener::bind(&addr).await?;
                info!("Server listening on http://{}", addr);
                info!("Admin API available at domain: {}", domain);
                axum::serve(listener, app).await?;
            }
        }

        Ok(())
    }
}
