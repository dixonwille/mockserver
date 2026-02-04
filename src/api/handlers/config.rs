// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Config handlers - reload configuration

use crate::server::AppState;
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use tracing::info;

/// POST /api/config/reload response
#[derive(Serialize, Deserialize)]
pub struct ReloadResponse {
    pub reloaded: Vec<String>,
}

/// POST /api/config/reload
pub async fn reload_config(
    State(state): State<AppState>,
) -> Result<Json<ReloadResponse>, (StatusCode, Json<serde_json::Value>)> {
    // Get currently loaded domains
    let loaded = state.scripts.loaded_domains();

    // Reload each domain
    let mut reloaded = Vec::new();
    for domain in &loaded {
        if let Err(e) = state.scripts.load_domain(domain).await {
            info!(domain = %domain, error = %e, "Failed to reload domain");
        } else {
            reloaded.push(domain.clone());
            info!(domain = %domain, "Reloaded domain");
        }
    }

    Ok(Json(ReloadResponse { reloaded }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::handlers::test_utils::test_state;
    use axum::{Router, body::Body, http::Request, routing::post};
    use tower::ServiceExt;

    /// Build the config API router for testing
    fn config_router(state: AppState) -> Router {
        Router::new()
            .route("/api/config/reload", post(reload_config))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_reload_config_no_domains() {
        let (state, _temp) = test_state().await;
        let app = config_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/config/reload")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 10000)
            .await
            .unwrap();
        let json: ReloadResponse = serde_json::from_slice(&body).unwrap();
        assert!(json.reloaded.is_empty());
    }
}
