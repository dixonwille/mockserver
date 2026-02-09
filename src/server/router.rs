// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Router construction and application state

use crate::config::ServerConfig;
use crate::lua::ScriptManager;
use axum::{Router, extract::DefaultBodyLimit, extract::Request};
use std::sync::Arc;
use tokio_rusqlite::Connection;
use tower::ServiceExt;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Database connection
    pub db: Arc<Connection>,
    /// Lua script manager
    pub scripts: Arc<ScriptManager>,
    /// Server configuration
    pub config: Arc<ServerConfig>,
}

impl AppState {
    /// Create new application state
    pub fn new(db: Connection, scripts: ScriptManager, config: ServerConfig) -> Self {
        Self {
            db: Arc::new(db),
            scripts: Arc::new(scripts),
            config: Arc::new(config),
        }
    }

    /// Create new application state from Arc-wrapped components
    pub fn from_arc(
        db: Arc<Connection>,
        scripts: Arc<ScriptManager>,
        config: Arc<ServerConfig>,
    ) -> Self {
        Self {
            db,
            scripts,
            config,
        }
    }
}

/// Build the mock server router
pub fn build_mock_router(state: AppState) -> Router {
    Router::new()
        // Catch-all handler for any method and path
        .fallback(super::handler::handle_mock_request)
        .layer(DefaultBodyLimit::max(state.config.max_body_size))
        .with_state(state)
}

/// Build a router that dispatches to `api_router` or `mock_router` based on
/// whether the request's Host header matches `api_domain`.
pub fn build_domain_dispatch_router(
    api_router: Router,
    mock_router: Router,
    api_domain: &str,
) -> Router {
    let api_domain = api_domain.to_lowercase();

    Router::new().fallback(move |request: Request| {
        let api = api_router.clone();
        let mock = mock_router.clone();
        let domain = api_domain.clone();
        async move {
            let host = super::handler::extract_domain(request.headers());
            let router = if host.as_deref() == Some(domain.as_str()) {
                api
            } else {
                mock
            };
            match router.oneshot(request).await {
                Ok(resp) => resp,
                Err(err) => match err {},
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ServerConfig;
    use crate::db::init_database;
    use crate::lua::ScriptManager;
    use tempfile::TempDir;

    async fn test_app_state() -> (AppState, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mocks_dir = temp_dir.path().join("mocks");
        std::fs::create_dir_all(&mocks_dir).unwrap();

        let db = init_database(":memory:", 2).await.unwrap();
        let config = ServerConfig {
            mocks_dir: mocks_dir.clone(),
            ..Default::default()
        };
        let scripts = ScriptManager::new(mocks_dir, config.lua_memory_mb, config.script_timeout);
        (AppState::new(db, scripts, config), temp_dir)
    }

    #[tokio::test]
    async fn test_app_state_new() {
        let (state, _temp) = test_app_state().await;
        // Verify fields are accessible
        assert_eq!(state.config.port, 3000);
    }

    #[tokio::test]
    async fn test_app_state_from_arc() {
        let (state, _temp) = test_app_state().await;
        let state2 = AppState::from_arc(
            state.db.clone(),
            state.scripts.clone(),
            state.config.clone(),
        );
        assert_eq!(state2.config.port, state.config.port);
    }

    #[tokio::test]
    async fn test_build_mock_router_has_body_limit() {
        let temp_dir = TempDir::new().unwrap();
        let mocks_dir = temp_dir.path().join("mocks");
        std::fs::create_dir_all(&mocks_dir).unwrap();

        let db = init_database(":memory:", 2).await.unwrap();
        let config = ServerConfig {
            mocks_dir: mocks_dir.clone(),
            max_body_size: 1024,
            ..Default::default()
        };
        let scripts = ScriptManager::new(mocks_dir, config.lua_memory_mb, config.script_timeout);
        let state = AppState::new(db, scripts, config);
        let _app = build_mock_router(state);
        // Router builds successfully with body limit configured
    }
}
