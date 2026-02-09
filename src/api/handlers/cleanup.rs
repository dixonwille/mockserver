// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Cleanup handlers - retention cleanup

use crate::api::handlers::DeleteResponse;
use crate::db::{cleanup_requests_before, delete_all_requests};
use crate::server::AppState;
use axum::{Json, extract::State, http::StatusCode};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use tracing::info;

#[derive(Deserialize)]
pub struct CleanupRequest {
    before: CleanupBefore,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum CleanupBefore {
    All(u8),
    Date(DateTime<Utc>),
}

/// POST /api/cleanup - Run cleanup with explicit `before` parameter
///
/// Body: `{"before": 0}` to delete all, or `{"before": "<ISO8601>"}` to delete before a date.
pub async fn cleanup_requests(
    State(state): State<AppState>,
    Json(body): Json<CleanupRequest>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = match body.before {
        CleanupBefore::All(0) => delete_all_requests(&state.db).await,
        CleanupBefore::All(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "error": "Invalid value",
                    "message": "Numeric 'before' must be 0 (to delete all)"
                })),
            ));
        }
        CleanupBefore::Date(cutoff) => cleanup_requests_before(&state.db, cutoff).await,
    }
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error", "message": e.to_string()})),
        )
    })?;

    info!(deleted, "Cleanup completed");
    Ok(Json(DeleteResponse { deleted }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::handlers::test_utils::{test_request_record, test_state};
    use crate::db::store_request;
    use axum::{Router, body::Body, http::Request, routing::post};
    use chrono::Duration;
    use tower::ServiceExt;

    fn cleanup_router(state: AppState) -> Router {
        Router::new()
            .route("/api/cleanup", post(cleanup_requests))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_cleanup_before_zero_deletes_all() {
        let (state, _temp) = test_state().await;

        // Store some requests
        let req = test_request_record("example.com", "GET", "/test");
        store_request(&state.db, &req).await.unwrap();

        let app = cleanup_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/cleanup")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"before": 0}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let json: DeleteResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.deleted, 1);
    }

    #[tokio::test]
    async fn test_cleanup_before_date_deletes_old() {
        let (state, _temp) = test_state().await;

        // Store a request and backdate it
        let req = test_request_record("example.com", "GET", "/old");
        store_request(&state.db, &req).await.unwrap();

        let old_time = Utc::now() - Duration::days(10);
        let req_id = req.id.to_string();
        state
            .db
            .call(move |conn| -> tokio_rusqlite::rusqlite::Result<()> {
                conn.execute(
                    "UPDATE requests SET received_at = ?1 WHERE id = ?2",
                    tokio_rusqlite::params![old_time.to_rfc3339(), req_id],
                )?;
                Ok(())
            })
            .await
            .unwrap();

        // Store a recent request
        let recent = test_request_record("example.com", "GET", "/new");
        store_request(&state.db, &recent).await.unwrap();

        let app = cleanup_router(state);

        // Delete everything before 5 days ago
        let cutoff = (Utc::now() - Duration::days(5)).to_rfc3339();
        let body_str = format!(r#"{{"before": "{}"}}"#, cutoff);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/cleanup")
                    .header("content-type", "application/json")
                    .body(Body::from(body_str))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let json: DeleteResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.deleted, 1);
    }

    #[tokio::test]
    async fn test_cleanup_no_body_returns_error() {
        let (state, _temp) = test_state().await;
        let app = cleanup_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/cleanup")
                    .header("content-type", "application/json")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // axum returns 422 for invalid/empty JSON body
        assert!(response.status().is_client_error());
    }

    #[tokio::test]
    async fn test_cleanup_nonzero_integer_returns_400() {
        let (state, _temp) = test_state().await;
        let app = cleanup_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/cleanup")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"before": 5}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}
