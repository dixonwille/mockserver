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
