// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Request handlers - list, get, get response, and delete

use crate::db::{
    RequestQuery, delete_all_requests, get_request_by_id, get_requests, get_response_by_request_id,
};
use crate::server::AppState;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::info;
use uuid::Uuid;

/// List requests response
#[derive(Serialize, Deserialize)]
pub struct ListRequestsResponse {
    pub requests: Vec<RequestSummary>,
    pub total: usize,
    pub limit: i64,
    pub offset: i64,
}

/// Summary of a request for list view
#[derive(Serialize, Deserialize)]
pub struct RequestSummary {
    pub id: String,
    pub domain: String,
    pub method: String,
    pub path: String,
    pub status: Option<u16>,
    pub received_at: String,
    pub duration_ms: Option<u64>,
}

/// Query parameters for list requests
#[derive(Debug, Deserialize)]
pub struct ListRequestsQuery {
    pub domain: Option<String>,
    pub method: Option<String>,
    pub path: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

/// DELETE /api/requests response
#[derive(Serialize, Deserialize)]
pub struct DeleteResponse {
    pub deleted: u64,
}

/// GET /api/requests
pub async fn list_requests(
    State(state): State<AppState>,
    Query(params): Query<ListRequestsQuery>,
) -> Result<Json<ListRequestsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let query = RequestQuery {
        domain: params.domain,
        method: params.method,
        path: params.path,
        since: None,
        limit: params.limit,
        offset: params.offset,
    };

    let limit = query.limit();
    let offset = query.offset();

    let requests = get_requests(&state.db, query).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error", "message": e.to_string()})),
        )
    })?;

    // Get response status for each request
    let mut summaries = Vec::with_capacity(requests.len());
    for req in &requests {
        let response = get_response_by_request_id(&state.db, req.id)
            .await
            .ok()
            .flatten();
        summaries.push(RequestSummary {
            id: req.id.to_string(),
            domain: req.domain.clone(),
            method: req.method.clone(),
            path: req.path.clone(),
            status: response.as_ref().map(|r| r.status_code),
            received_at: req.received_at.to_rfc3339(),
            duration_ms: response.map(|r| r.duration_ms),
        });
    }

    Ok(Json(ListRequestsResponse {
        total: summaries.len(),
        requests: summaries,
        limit,
        offset,
    }))
}

/// GET /api/requests/:id
pub async fn get_request(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let uuid = Uuid::parse_str(&id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(
                json!({"error": "Invalid UUID", "message": "The provided ID is not a valid UUID"}),
            ),
        )
    })?;

    let request = get_request_by_id(&state.db, uuid)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error", "message": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Not found", "message": "Request not found"})),
            )
        })?;

    Ok(Json(request))
}

/// GET /api/requests/:id/response
pub async fn get_request_response(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let uuid = Uuid::parse_str(&id).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(
                json!({"error": "Invalid UUID", "message": "The provided ID is not a valid UUID"}),
            ),
        )
    })?;

    // First verify the request exists
    get_request_by_id(&state.db, uuid)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error", "message": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({"error": "Not found", "message": "Request not found"})),
            )
        })?;

    let response = get_response_by_request_id(&state.db, uuid)
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"error": "Database error", "message": e.to_string()})),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(
                    json!({"error": "Not found", "message": "Response not found for this request"}),
                ),
            )
        })?;

    Ok(Json(response))
}

/// DELETE /api/requests
pub async fn delete_requests(
    State(state): State<AppState>,
) -> Result<Json<DeleteResponse>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_all_requests(&state.db).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "Database error", "message": e.to_string()})),
        )
    })?;

    info!(deleted = deleted, "Deleted all requests");

    Ok(Json(DeleteResponse { deleted }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::handlers::test_utils::{test_request_record, test_response_record, test_state};
    use crate::db::{RequestRecord, ResponseRecord, store_request, store_response};
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use tower::ServiceExt;

    /// Build the requests API router for testing
    fn requests_router(state: AppState) -> Router {
        Router::new()
            .route("/api/requests", get(list_requests).delete(delete_requests))
            .route("/api/requests/{id}", get(get_request))
            .route("/api/requests/{id}/response", get(get_request_response))
            .with_state(state)
    }

    // ==================== list_requests tests ====================

    #[tokio::test]
    async fn test_list_requests_empty() {
        let (state, _temp) = test_state().await;
        let app = requests_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/requests")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let json: ListRequestsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.total, 0);
        assert!(json.requests.is_empty());
    }

    #[tokio::test]
    async fn test_list_requests_returns_stored() {
        let (state, _temp) = test_state().await;

        // Store some requests
        let req1 = test_request_record("example.com", "GET", "/api/test");
        let req2 = test_request_record("example.com", "POST", "/api/data");
        store_request(&state.db, &req1).await.unwrap();
        store_request(&state.db, &req2).await.unwrap();

        let app = requests_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/requests")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let json: ListRequestsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.total, 2);
    }

    #[tokio::test]
    async fn test_list_requests_filter_by_domain() {
        let (state, _temp) = test_state().await;

        store_request(&state.db, &test_request_record("example.com", "GET", "/a"))
            .await
            .unwrap();
        store_request(&state.db, &test_request_record("other.com", "GET", "/b"))
            .await
            .unwrap();

        let app = requests_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/requests?domain=example.com")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let json: ListRequestsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.total, 1);
        assert_eq!(json.requests[0].domain, "example.com");
    }

    #[tokio::test]
    async fn test_list_requests_includes_response_status() {
        let (state, _temp) = test_state().await;

        let req = test_request_record("example.com", "GET", "/test");
        store_request(&state.db, &req).await.unwrap();

        let resp = test_response_record(req.id);
        store_response(&state.db, &resp).await.unwrap();

        let app = requests_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/requests")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let json: ListRequestsResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.requests[0].status, Some(200));
        assert_eq!(json.requests[0].duration_ms, Some(42));
    }

    // ==================== get_request tests ====================

    #[tokio::test]
    async fn test_get_request_by_id_success() {
        let (state, _temp) = test_state().await;

        let req = test_request_record("example.com", "GET", "/test");
        store_request(&state.db, &req).await.unwrap();

        let app = requests_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/requests/{}", req.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let json: RequestRecord = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.id, req.id);
        assert_eq!(json.domain, "example.com");
    }

    #[tokio::test]
    async fn test_get_request_by_id_not_found() {
        let (state, _temp) = test_state().await;
        let app = requests_router(state);

        let random_id = Uuid::new_v4();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/requests/{}", random_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_request_by_id_invalid_uuid() {
        let (state, _temp) = test_state().await;
        let app = requests_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/requests/not-a-uuid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    // ==================== get_request_response tests ====================

    #[tokio::test]
    async fn test_get_request_response_success() {
        let (state, _temp) = test_state().await;

        let req = test_request_record("example.com", "GET", "/test");
        store_request(&state.db, &req).await.unwrap();

        let resp = test_response_record(req.id);
        store_response(&state.db, &resp).await.unwrap();

        let app = requests_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/requests/{}/response", req.id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let json: ResponseRecord = serde_json::from_slice(&body).unwrap();
        assert_eq!(json.request_id, req.id);
        assert_eq!(json.status_code, 200);
    }

    #[tokio::test]
    async fn test_get_request_response_request_not_found() {
        let (state, _temp) = test_state().await;
        let app = requests_router(state);

        let random_id = Uuid::new_v4();
        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/requests/{}/response", random_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // ==================== delete_requests tests ====================

    #[tokio::test]
    async fn test_delete_requests_success() {
        let (state, _temp) = test_state().await;

        store_request(&state.db, &test_request_record("example.com", "GET", "/a"))
            .await
            .unwrap();
        store_request(&state.db, &test_request_record("example.com", "GET", "/b"))
            .await
            .unwrap();

        let app = requests_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/requests")
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
        assert_eq!(json.deleted, 2);
    }

    #[tokio::test]
    async fn test_delete_requests_empty_database() {
        let (state, _temp) = test_state().await;
        let app = requests_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/requests")
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
        assert_eq!(json.deleted, 0);
    }
}
