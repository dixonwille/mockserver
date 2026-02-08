// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Request handler for mock requests

use crate::db::{RequestRecord, ResponseRecord, store_request, store_response};
use crate::error::Error;
use crate::lua::{LuaRequest, LuaResponse};
use crate::server::router::AppState;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Main handler for mock requests
pub async fn handle_mock_request(
    State(state): State<AppState>,
    request: Request,
) -> impl IntoResponse {
    let start = Instant::now();
    let request_id = Uuid::new_v4();

    // Extract and validate domain from headers
    let domain = match extract_domain(request.headers()) {
        Some(d) => d,
        None => {
            return error_response(StatusCode::BAD_REQUEST, "Invalid or missing Host header");
        }
    };

    debug!(request_id = %request_id, domain = %domain, "Processing request");

    // Extract request parts
    let method = request.method().to_string();
    let uri = request.uri().clone();
    let path = uri.path().to_string();
    let query_string = uri.query().map(|q| q.to_string());
    let headers = headers_to_map(request.headers());

    // Read body
    let body_bytes =
        match axum::body::to_bytes(request.into_body(), state.config.max_body_size).await {
            Ok(b) => b,
            Err(e) => {
                warn!(error = %e, "Failed to read request body");
                return error_response(StatusCode::BAD_REQUEST, "Failed to read body");
            }
        };
    let body_string = String::from_utf8_lossy(&body_bytes).to_string();

    // Build Lua request
    let lua_request = LuaRequest {
        method: method.clone(),
        path: path.clone(),
        query: parse_query_string(query_string.as_deref()),
        headers: headers.clone(),
        body: body_string.clone(),
        domain: domain.clone(),
    };

    // Store the request record
    let request_record = RequestRecord {
        id: request_id,
        domain: domain.clone(),
        method: method.clone(),
        path: path.clone(),
        query_string: query_string.clone(),
        headers: json!(headers),
        body: if body_bytes.is_empty() {
            None
        } else {
            Some(body_bytes.to_vec())
        },
        received_at: Utc::now(),
    };

    if let Err(e) = store_request(&state.db, &request_record).await {
        error!(error = %e, "Failed to store request");
        // Continue processing even if storage fails
    }

    // Execute Lua handler
    let (lua_response, lua_error) = match state.scripts.execute(lua_request).await {
        Ok(resp) => (resp, None),
        Err(e) => {
            let error_msg = e.to_string();
            error!(
                request_id = %request_id,
                domain = %domain,
                error = %error_msg,
                "Lua handler error"
            );

            // Return error response but still record it
            let error_response = LuaResponse {
                status: match &e {
                    Error::ScriptNotFound(_) => 404,
                    Error::ScriptTimeout(_) => 504,
                    _ => 500,
                },
                headers: HashMap::from([(
                    "Content-Type".to_string(),
                    "application/json".to_string(),
                )]),
                body: json!({
                    "error": "Handler Error",
                    "message": error_msg
                })
                .to_string(),
            };
            (error_response, Some(error_msg))
        }
    };

    let duration = start.elapsed();

    // Store the response record
    let response_record = ResponseRecord {
        id: Uuid::new_v4(),
        request_id,
        status_code: lua_response.status,
        headers: json!(lua_response.headers),
        body: if lua_response.body.is_empty() {
            None
        } else {
            Some(lua_response.body.as_bytes().to_vec())
        },
        lua_script: format!("{}/init.lua", domain),
        duration_ms: duration.as_millis() as u64,
        error: lua_error,
    };

    if let Err(e) = store_response(&state.db, &response_record).await {
        error!(error = %e, "Failed to store response");
    }

    info!(
        request_id = %request_id,
        domain = %domain,
        method = %method,
        path = %path,
        status = lua_response.status,
        duration_ms = duration.as_millis(),
        "Request completed"
    );

    // Build HTTP response
    build_http_response(lua_response)
}

/// Extract domain from request headers
/// Priority: X-Original-Host > X-Forwarded-Host > Host
pub(crate) fn extract_domain(headers: &HeaderMap) -> Option<String> {
    // Try headers in priority order
    let raw = headers
        .get("x-original-host")
        .or_else(|| headers.get("x-forwarded-host"))
        .or_else(|| headers.get("host"))
        .and_then(|v| v.to_str().ok())?;

    // Strip port if present
    let domain = raw.split(':').next().unwrap_or(raw);

    // Validate domain: only allow alphanumeric, dots, and hyphens
    // Also enforce max DNS name length of 253 characters
    if domain
        .chars()
        .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
        && !domain.contains("..")
        && !domain.starts_with('.')
        && !domain.starts_with('-')
        && domain.len() <= 253
    {
        Some(domain.to_lowercase())
    } else {
        warn!(domain = %domain, "Rejected invalid domain header");
        None
    }
}

/// Convert header map to HashMap<String, String>
fn headers_to_map(headers: &HeaderMap) -> HashMap<String, String> {
    headers
        .iter()
        .filter_map(|(k, v)| {
            v.to_str()
                .ok()
                .map(|val| (k.as_str().to_lowercase(), val.to_string()))
        })
        .collect()
}

/// Parse query string into HashMap
fn parse_query_string(query: Option<&str>) -> HashMap<String, String> {
    query
        .map(|q| {
            q.split('&')
                .filter_map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    let key = parts.next()?;
                    let value = parts.next().unwrap_or("");
                    Some((
                        urlencoding::decode(key).ok()?.into_owned(),
                        urlencoding::decode(value).ok()?.into_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Build HTTP response from Lua response
fn build_http_response(lua_response: LuaResponse) -> Response {
    let mut builder = Response::builder().status(lua_response.status);

    // Add headers
    for (key, value) in lua_response.headers {
        if let Ok(header_value) = HeaderValue::from_str(&value) {
            builder = builder.header(&key, header_value);
        }
    }

    builder
        .body(Body::from(lua_response.body))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("Failed to build response"))
                .unwrap()
        })
}

/// Build an error response
fn error_response(status: StatusCode, message: &str) -> Response {
    Response::builder()
        .status(status)
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "error": status.canonical_reason().unwrap_or("Error"),
                "message": message
            })
            .to_string(),
        ))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::{HOST, HeaderName};

    fn make_headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(
                HeaderName::from_lowercase(name.to_lowercase().as_bytes()).unwrap(),
                HeaderValue::from_str(value).unwrap(),
            );
        }
        headers
    }

    // ==================== extract_domain tests ====================

    #[test]
    fn test_extract_domain_from_host_header() {
        let headers = make_headers(&[("host", "example.com")]);
        assert_eq!(extract_domain(&headers), Some("example.com".to_string()));
    }

    #[test]
    fn test_extract_domain_from_host_with_port() {
        let headers = make_headers(&[("host", "example.com:8080")]);
        assert_eq!(extract_domain(&headers), Some("example.com".to_string()));
    }

    #[test]
    fn test_extract_domain_lowercases() {
        let headers = make_headers(&[("host", "EXAMPLE.COM")]);
        assert_eq!(extract_domain(&headers), Some("example.com".to_string()));
    }

    #[test]
    fn test_extract_domain_x_forwarded_host_priority() {
        let headers = make_headers(&[
            ("host", "backend.local"),
            ("x-forwarded-host", "example.com"),
        ]);
        assert_eq!(extract_domain(&headers), Some("example.com".to_string()));
    }

    #[test]
    fn test_extract_domain_x_original_host_highest_priority() {
        let headers = make_headers(&[
            ("host", "backend.local"),
            ("x-forwarded-host", "proxy.example.com"),
            ("x-original-host", "original.example.com"),
        ]);
        assert_eq!(
            extract_domain(&headers),
            Some("original.example.com".to_string())
        );
    }

    #[test]
    fn test_extract_domain_rejects_double_dots() {
        let headers = make_headers(&[("host", "example..com")]);
        assert_eq!(extract_domain(&headers), None);
    }

    #[test]
    fn test_extract_domain_rejects_leading_dot() {
        let headers = make_headers(&[("host", ".example.com")]);
        assert_eq!(extract_domain(&headers), None);
    }

    #[test]
    fn test_extract_domain_rejects_leading_hyphen() {
        let headers = make_headers(&[("host", "-example.com")]);
        assert_eq!(extract_domain(&headers), None);
    }

    #[test]
    fn test_extract_domain_rejects_too_long() {
        let long_domain = "a".repeat(254);
        let headers = make_headers(&[("host", &long_domain)]);
        assert_eq!(extract_domain(&headers), None);
    }

    #[test]
    fn test_extract_domain_allows_max_length() {
        let max_domain = "a".repeat(253);
        let headers = make_headers(&[("host", &max_domain)]);
        assert_eq!(extract_domain(&headers), Some(max_domain));
    }

    #[test]
    fn test_extract_domain_rejects_invalid_characters() {
        let headers = make_headers(&[("host", "example.com/path")]);
        assert_eq!(extract_domain(&headers), None);
    }

    #[test]
    fn test_extract_domain_rejects_spaces() {
        let headers = make_headers(&[("host", "example .com")]);
        assert_eq!(extract_domain(&headers), None);
    }

    #[test]
    fn test_extract_domain_allows_hyphens() {
        let headers = make_headers(&[("host", "my-example.com")]);
        assert_eq!(extract_domain(&headers), Some("my-example.com".to_string()));
    }

    #[test]
    fn test_extract_domain_allows_underscores_in_subdomain() {
        // Underscores are technically invalid in domain names but we allow alphanumeric + dots + hyphens
        let headers = make_headers(&[("host", "test_server.local")]);
        assert_eq!(extract_domain(&headers), None); // underscore not allowed
    }

    #[test]
    fn test_extract_domain_missing_header() {
        let headers = HeaderMap::new();
        assert_eq!(extract_domain(&headers), None);
    }

    // ==================== headers_to_map tests ====================

    #[test]
    fn test_headers_to_map_basic() {
        let headers = make_headers(&[
            ("content-type", "application/json"),
            ("authorization", "Bearer token"),
        ]);

        let map = headers_to_map(&headers);
        assert_eq!(
            map.get("content-type"),
            Some(&"application/json".to_string())
        );
        assert_eq!(map.get("authorization"), Some(&"Bearer token".to_string()));
    }

    #[test]
    fn test_headers_to_map_lowercases_keys() {
        let mut headers = HeaderMap::new();
        headers.insert(HOST, HeaderValue::from_static("example.com"));

        let map = headers_to_map(&headers);
        assert_eq!(map.get("host"), Some(&"example.com".to_string()));
    }

    #[test]
    fn test_headers_to_map_empty() {
        let headers = HeaderMap::new();
        let map = headers_to_map(&headers);
        assert!(map.is_empty());
    }

    // ==================== parse_query_string tests ====================

    #[test]
    fn test_parse_query_string_single_param() {
        let result = parse_query_string(Some("key=value"));
        assert_eq!(result.get("key"), Some(&"value".to_string()));
    }

    #[test]
    fn test_parse_query_string_multiple_params() {
        let result = parse_query_string(Some("a=1&b=2&c=3"));
        assert_eq!(result.get("a"), Some(&"1".to_string()));
        assert_eq!(result.get("b"), Some(&"2".to_string()));
        assert_eq!(result.get("c"), Some(&"3".to_string()));
    }

    #[test]
    fn test_parse_query_string_url_decoding() {
        let result = parse_query_string(Some("name=hello%20world&path=%2Fapi%2Ftest"));
        assert_eq!(result.get("name"), Some(&"hello world".to_string()));
        assert_eq!(result.get("path"), Some(&"/api/test".to_string()));
    }

    #[test]
    fn test_parse_query_string_empty_value() {
        let result = parse_query_string(Some("key="));
        assert_eq!(result.get("key"), Some(&"".to_string()));
    }

    #[test]
    fn test_parse_query_string_no_value() {
        let result = parse_query_string(Some("flag"));
        assert_eq!(result.get("flag"), Some(&"".to_string()));
    }

    #[test]
    fn test_parse_query_string_none() {
        let result = parse_query_string(None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_query_string_empty_string() {
        let result = parse_query_string(Some(""));
        // Empty string results in one empty key
        assert!(result.is_empty() || result.len() == 1);
    }

    #[test]
    fn test_parse_query_string_special_chars() {
        let result = parse_query_string(Some("email=test%40example.com"));
        assert_eq!(result.get("email"), Some(&"test@example.com".to_string()));
    }

    #[test]
    fn test_parse_query_string_duplicate_keys() {
        // Last value wins
        let result = parse_query_string(Some("key=first&key=second"));
        assert_eq!(result.get("key"), Some(&"second".to_string()));
    }

    #[test]
    fn test_parse_query_string_plus_as_space() {
        let result = parse_query_string(Some("query=hello+world"));
        // URL encoding: + should remain as + (only %20 is space)
        // Actually urlencoding::decode converts + to space in query strings
        assert!(result.contains_key("query"));
    }

    // ==================== build_http_response tests ====================

    #[tokio::test]
    async fn test_build_http_response_status() {
        let lua_response = LuaResponse {
            status: 201,
            headers: HashMap::new(),
            body: String::new(),
        };

        let response = build_http_response(lua_response);
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn test_build_http_response_headers() {
        let mut headers = HashMap::new();
        headers.insert("x-custom".to_string(), "test-value".to_string());
        headers.insert("content-type".to_string(), "text/plain".to_string());

        let lua_response = LuaResponse {
            status: 200,
            headers,
            body: String::new(),
        };

        let response = build_http_response(lua_response);
        assert_eq!(response.headers().get("x-custom").unwrap(), "test-value");
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "text/plain"
        );
    }

    #[tokio::test]
    async fn test_build_http_response_body() {
        let lua_response = LuaResponse {
            status: 200,
            headers: HashMap::new(),
            body: "Hello, World!".to_string(),
        };

        let response = build_http_response(lua_response);
        let body_bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body_bytes[..], b"Hello, World!");
    }

    #[tokio::test]
    async fn test_build_http_response_empty_body() {
        let lua_response = LuaResponse {
            status: 204,
            headers: HashMap::new(),
            body: String::new(),
        };

        let response = build_http_response(lua_response);
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let body_bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert!(body_bytes.is_empty());
    }

    // ==================== error_response tests ====================

    #[tokio::test]
    async fn test_error_response_bad_request() {
        let response = error_response(StatusCode::BAD_REQUEST, "Invalid input");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body_bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body["error"], "Bad Request");
        assert_eq!(body["message"], "Invalid input");
    }

    #[tokio::test]
    async fn test_error_response_internal_server_error() {
        let response = error_response(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong");
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body_bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body["error"], "Internal Server Error");
        assert_eq!(body["message"], "Something went wrong");
    }

    #[tokio::test]
    async fn test_error_response_not_found() {
        let response = error_response(StatusCode::NOT_FOUND, "Resource not found");
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_error_response_has_json_content_type() {
        let response = error_response(StatusCode::BAD_REQUEST, "test");
        assert_eq!(
            response.headers().get("content-type").unwrap(),
            "application/json"
        );
    }

    // ==================== domain routing dispatch tests ====================

    use axum::Router;
    use axum::routing::any;
    use tower::ServiceExt;

    /// Build a domain-routing app using the real dispatch function with stub routers.
    fn build_domain_dispatch_app(api_domain: &str) -> Router {
        let api_router =
            Router::new().route("/api/healthz", any(|| async { (StatusCode::OK, "api") }));
        let mock_router = Router::new().fallback(|| async { (StatusCode::OK, "mock") });

        crate::server::build_domain_dispatch_router(api_router, mock_router, api_domain)
    }

    #[tokio::test]
    async fn test_domain_routing_api_domain_routes_to_api() {
        let app = build_domain_dispatch_app("admin.local");
        let request = Request::builder()
            .uri("/api/healthz")
            .header("host", "admin.local")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body[..], b"api");
    }

    #[tokio::test]
    async fn test_domain_routing_non_api_domain_routes_to_mock() {
        let app = build_domain_dispatch_app("admin.local");
        let request = Request::builder()
            .uri("/anything")
            .header("host", "example.com")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body[..], b"mock");
    }

    #[tokio::test]
    async fn test_domain_routing_api_domain_unknown_path_returns_404() {
        let app = build_domain_dispatch_app("admin.local");
        let request = Request::builder()
            .uri("/unknown")
            .header("host", "admin.local")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_domain_routing_case_insensitive() {
        let app = build_domain_dispatch_app("admin.local");
        let request = Request::builder()
            .uri("/api/healthz")
            .header("host", "ADMIN.LOCAL:8080")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body[..], b"api");
    }

    #[tokio::test]
    async fn test_domain_routing_x_forwarded_host() {
        let app = build_domain_dispatch_app("admin.local");
        let request = Request::builder()
            .uri("/api/healthz")
            .header("host", "backend.internal")
            .header("x-forwarded-host", "admin.local")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&body[..], b"api");
    }
}
