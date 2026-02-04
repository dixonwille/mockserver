// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Admin API handlers

mod cleanup;
mod config;
mod health;
mod requests;

pub use cleanup::cleanup_requests;
pub use config::{ReloadResponse, reload_config};
pub use health::{AboutResponse, HealthResponse, about, health_check};
pub use requests::{
    DeleteResponse, ListRequestsQuery, ListRequestsResponse, RequestSummary, delete_requests,
    get_request, get_request_response, list_requests,
};

#[cfg(test)]
pub(crate) mod test_utils {
    use crate::config::ServerConfig;
    use crate::db::{RequestRecord, ResponseRecord, init_database};
    use crate::lua::ScriptManager;
    use crate::server::AppState;
    use chrono::Utc;
    use serde_json::json;
    use tempfile::TempDir;
    use uuid::Uuid;

    /// Create a test AppState with in-memory database
    pub async fn test_state() -> (AppState, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let mocks_dir = temp_dir.path().join("mocks");
        std::fs::create_dir_all(&mocks_dir).unwrap();

        let db = init_database(":memory:", 2).await.unwrap();
        let config = ServerConfig {
            mocks_dir: mocks_dir.clone(),
            retention_days: 7,
            ..Default::default()
        };
        let scripts = ScriptManager::new(mocks_dir, config.lua_memory_mb, config.script_timeout);

        let state = AppState::new(db, scripts, config);
        (state, temp_dir)
    }

    /// Helper to create a test request record
    pub fn test_request_record(domain: &str, method: &str, path: &str) -> RequestRecord {
        RequestRecord {
            id: Uuid::new_v4(),
            domain: domain.to_string(),
            method: method.to_string(),
            path: path.to_string(),
            query_string: None,
            headers: json!({"content-type": "application/json"}),
            body: None,
            received_at: Utc::now(),
        }
    }

    /// Helper to create a test response record
    pub fn test_response_record(request_id: Uuid) -> ResponseRecord {
        ResponseRecord {
            id: Uuid::new_v4(),
            request_id,
            status_code: 200,
            headers: json!({"content-type": "application/json"}),
            body: Some(b"test".to_vec()),
            lua_script: "test/init.lua".to_string(),
            duration_ms: 42,
            error: None,
        }
    }
}
