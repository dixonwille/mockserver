// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Database models for stored requests and responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A recorded HTTP request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestRecord {
    pub id: Uuid,
    pub domain: String,
    pub method: String,
    pub path: String,
    pub query_string: Option<String>,
    pub headers: serde_json::Value,
    pub body: Option<Vec<u8>>,
    pub received_at: DateTime<Utc>,
}

/// A recorded HTTP response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseRecord {
    pub id: Uuid,
    pub request_id: Uuid,
    pub status_code: u16,
    pub headers: serde_json::Value,
    pub body: Option<Vec<u8>>,
    pub lua_script: String,
    pub duration_ms: u64,
    pub error: Option<String>,
}

/// Query parameters for filtering requests
#[derive(Debug, Default, Deserialize)]
pub struct RequestQuery {
    pub domain: Option<String>,
    pub path: Option<String>,
    pub method: Option<String>,
    pub since: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

impl RequestQuery {
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(50).min(500)
    }

    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0)
    }
}
