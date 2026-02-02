// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Router construction and application state

use crate::config::ServerConfig;
use crate::lua::ScriptManager;
use axum::{Router, extract::DefaultBodyLimit};
use std::sync::Arc;
use tokio_rusqlite::Connection;

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
