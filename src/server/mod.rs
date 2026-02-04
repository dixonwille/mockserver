// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! HTTP server module
//!
//! Axum-based HTTP server for handling mock requests.

mod handler;
mod router;

pub use handler::handle_mock_request;
pub use router::{AppState, build_mock_router};
