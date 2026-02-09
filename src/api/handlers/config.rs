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
    use crate::config::ServerConfig;
    use crate::db::init_database;
    use crate::lua::ScriptManager;
    use axum::{Router, body::Body, http::Request, routing::post};
    use tempfile::TempDir;
    use tower::ServiceExt;

    /// Build the config API router for testing
    fn config_router(state: AppState) -> Router {
        Router::new()
            .route("/api/config/reload", post(reload_config))
            .with_state(state)
    }

    /// Create a test state with real domain directories for config reload tests
    async fn test_state_with_domains(domains: &[(&str, &str)]) -> (AppState, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mocks_dir = temp_dir.path().join("mocks");
        std::fs::create_dir_all(&mocks_dir).unwrap();

        for &(name, script) in domains {
            let domain_dir = mocks_dir.join(name);
            std::fs::create_dir_all(&domain_dir).unwrap();
            std::fs::write(domain_dir.join("init.lua"), script).unwrap();
        }

        let db = init_database(":memory:", 2).await.unwrap();
        let config = ServerConfig {
            mocks_dir: mocks_dir.clone(),
            ..Default::default()
        };
        let scripts = ScriptManager::new(mocks_dir, config.lua_memory_mb, config.script_timeout);
        let state = AppState::new(db, scripts, config);
        (state, temp_dir)
    }

    #[tokio::test]
    async fn test_reload_config_no_domains() {
        let (state, _temp) = test_state_with_domains(&[]).await;
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

    #[tokio::test]
    async fn test_reload_config_with_domains() {
        let script = "function handle(r) return {status=200} end";
        let (state, _temp) =
            test_state_with_domains(&[("_default", script), ("example.com", script)]).await;

        // Load domains first
        state.scripts.load_domain("_default").await.unwrap();
        state.scripts.load_domain("example.com").await.unwrap();

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
        assert_eq!(json.reloaded.len(), 2);
        let mut reloaded = json.reloaded.clone();
        reloaded.sort();
        assert!(reloaded.contains(&"_default".to_string()));
        assert!(reloaded.contains(&"example.com".to_string()));
    }

    #[tokio::test]
    async fn test_reload_config_broken_domain_skipped() {
        let script = "function handle(r) return {status=200} end";
        let (state, temp) =
            test_state_with_domains(&[("_default", script), ("broken.com", script)]).await;

        // Load domains first
        state.scripts.load_domain("_default").await.unwrap();
        state.scripts.load_domain("broken.com").await.unwrap();

        // Now corrupt broken.com's init.lua
        let broken_init = temp.path().join("mocks/broken.com/init.lua");
        std::fs::write(&broken_init, "this is {{not valid}} lua").unwrap();

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
        // Only _default should have reloaded successfully
        assert!(json.reloaded.contains(&"_default".to_string()));
        assert!(!json.reloaded.contains(&"broken.com".to_string()));
    }
}
