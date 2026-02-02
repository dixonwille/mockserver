// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Admin API router

use crate::api::handlers;
use crate::server::AppState;
use axum::{
    Router,
    routing::{delete, get, post},
};

/// Build the Admin API router
///
/// Endpoints:
/// - GET  /api/health           - Health check
/// - GET  /api/requests         - List requests (with filtering)
/// - GET  /api/requests/{id}     - Get a specific request
/// - GET  /api/requests/{id}/response - Get the response for a request
/// - DELETE /api/requests       - Delete all requests
/// - POST /api/config/reload    - Reload Lua scripts
/// - POST /api/cleanup          - Run retention cleanup
pub fn build_api_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(handlers::health_check))
        .route("/api/requests", get(handlers::list_requests))
        .route("/api/requests", delete(handlers::delete_requests))
        .route("/api/requests/{id}", get(handlers::get_request))
        .route(
            "/api/requests/{id}/response",
            get(handlers::get_request_response),
        )
        .route("/api/config/reload", post(handlers::reload_config))
        .route("/api/cleanup", post(handlers::cleanup_requests))
        .with_state(state)
}

/// Build a combined router that serves both mock and API on the same port
/// with API routes under a path prefix
pub fn build_combined_router(state: AppState, api_prefix: &str) -> Router {
    let api_routes = Router::new()
        .route("/api/health", get(handlers::health_check))
        .route("/api/requests", get(handlers::list_requests))
        .route("/api/requests", delete(handlers::delete_requests))
        .route("/api/requests/{id}", get(handlers::get_request))
        .route(
            "/api/requests/{id}/response",
            get(handlers::get_request_response),
        )
        .route("/api/config/reload", post(handlers::reload_config))
        .route("/api/cleanup", post(handlers::cleanup_requests));

    Router::new().nest(api_prefix, api_routes).with_state(state)
}
