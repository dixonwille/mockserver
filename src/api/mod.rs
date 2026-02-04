// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Admin API module
//!
//! REST API for querying stored requests and server management.

mod handlers;
mod router;

pub use handlers::{
    AboutResponse, DeleteResponse, HealthResponse, ListRequestsQuery, ListRequestsResponse,
    ReloadResponse, RequestSummary, about, cleanup_requests, delete_requests, get_request,
    get_request_response, health_check, list_requests, reload_config,
};
pub use router::{build_api_router, build_combined_router};
