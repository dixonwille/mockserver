// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Cleanup handlers - retention cleanup

use crate::api::handlers::DeleteResponse;
use crate::db::cleanup_old_requests;
use crate::server::AppState;
use axum::{Json, extract::State, http::StatusCode};
use serde_json::json;
use tracing::info;

/// POST /api/cleanup - Run retention cleanup
pub async fn cleanup_requests(
    State(state): State<AppState>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = cleanup_old_requests(&state.db, state.config.retention_days)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error", "message": e.to_string()})),
            )
        })?;

    info!(
        deleted = deleted,
        retention_days = state.config.retention_days,
        "Cleaned up old requests"
    );

    Ok(Json(DeleteResponse { deleted }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::handlers::test_utils::test_state;
    use axum::{Router, body::Body, http::Request, routing::post};
    use tower::ServiceExt;

    /// Build the cleanup API router for testing
    fn cleanup_router(state: AppState) -> Router {
        Router::new()
            .route("/api/cleanup", post(cleanup_requests))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_cleanup_requests_success() {
        let (state, _temp) = test_state().await;
        let app = cleanup_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/cleanup")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let json: DeleteResponse = serde_json::from_slice(&body).unwrap();
        // All requests are recent, so none should be deleted
        assert_eq!(json.deleted, 0);
    }
}
