// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Admin API router

use crate::api::handlers;
use crate::server::AppState;
use axum::{
    Router,
    routing::{delete, get, post},
};

/// API route definitions shared by all routing modes.
fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/api/healthz", get(handlers::health_check))
        .route("/api/about", get(handlers::about))
        .route("/api/requests", get(handlers::list_requests))
        .route("/api/requests", delete(handlers::delete_requests))
        .route("/api/requests/{id}", get(handlers::get_request))
        .route(
            "/api/requests/{id}/response",
            get(handlers::get_request_response),
        )
        .route("/api/config/reload", post(handlers::reload_config))
        .route("/api/cleanup", post(handlers::cleanup_requests))
}

/// Build the Admin API router
///
/// Endpoints:
/// - GET  /api/healthz          - Health check
/// - GET  /api/about            - License and source information
/// - GET  /api/requests         - List requests (with filtering)
/// - GET  /api/requests/{id}     - Get a specific request
/// - GET  /api/requests/{id}/response - Get the response for a request
/// - DELETE /api/requests       - Delete all requests
/// - POST /api/config/reload    - Reload Lua scripts
/// - POST /api/cleanup          - Run retention cleanup
pub fn build_api_router(state: AppState) -> Router {
    api_routes().with_state(state)
}

/// Build a combined router that serves both mock and API on the same port
/// with API routes under a path prefix
pub fn build_api_prefixed_router(state: AppState, api_prefix: &str) -> Router {
    Router::new()
        .nest(api_prefix, api_routes())
        .with_state(state)
}
