// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Database queries and connection management

use crate::db::migrations;
use crate::db::models::{RequestQuery, RequestRecord, ResponseRecord};
use crate::error::Result;
use chrono::{DateTime, Duration, Utc};
use tokio_rusqlite::{Connection, OptionalExtension, Row, params};
use tracing::info;
use uuid::Uuid;

/// Initialize the database with schema and pragmas
pub async fn init_database(path: &str, cache_size_mb: u32) -> Result<Connection> {
    let conn = Connection::open(path).await?;

    // Convert MB to negative KB (SQLite convention)
    let cache_size_kb = -(cache_size_mb as i64 * 1024);

    conn.call(move |conn| -> tokio_rusqlite::rusqlite::Result<()> {
        // Configure pragmas for performance (must be set before migrations)
        conn.execute_batch(&format!(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA busy_timeout=5000;
             PRAGMA cache_size={cache_size_kb};
             PRAGMA foreign_keys=ON;"
        ))?;

        // Apply any pending migrations
        migrations::migrate(conn).map_err(|e| {
            tokio_rusqlite::rusqlite::Error::ToSqlConversionFailure(Box::new(
                std::io::Error::other(format!("Migration failed: {e}")),
            ))
        })?;

        let version = migrations::current_version(conn).unwrap_or(0);
        info!(schema_version = version, "Database initialized");

        Ok(())
    })
    .await?;

    Ok(conn)
}

/// Store a new request record
pub async fn store_request(conn: &Connection, request: &RequestRecord) -> Result<()> {
    let request = request.clone();
    conn.call(move |conn| -> tokio_rusqlite::rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO requests (id, domain, method, path, query_string, headers, body, received_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                request.id.to_string(),
                request.domain,
                request.method,
                request.path,
                request.query_string,
                request.headers.to_string(),
                request.body,
                request.received_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    })
    .await?;
    Ok(())
}

/// Store a new response record
pub async fn store_response(conn: &Connection, response: &ResponseRecord) -> Result<()> {
    let response = response.clone();
    conn.call(move |conn| -> tokio_rusqlite::rusqlite::Result<()> {
        conn.execute(
            "INSERT INTO responses (id, request_id, status_code, headers, body, lua_script, duration_ms, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                response.id.to_string(),
                response.request_id.to_string(),
                response.status_code,
                response.headers.to_string(),
                response.body,
                response.lua_script,
                response.duration_ms as i64,
                response.error,
            ],
        )?;
        Ok(())
    })
    .await?;
    Ok(())
}

/// Helper to parse a request row into RequestRecord
fn request_from_row(row: &Row) -> tokio_rusqlite::rusqlite::Result<RequestRecord> {
    let id: String = row.get(0)?;
    let headers: String = row.get(5)?;
    let received_at: String = row.get(7)?;

    Ok(RequestRecord {
        id: Uuid::parse_str(&id).unwrap_or_default(),
        domain: row.get(1)?,
        method: row.get(2)?,
        path: row.get(3)?,
        query_string: row.get(4)?,
        headers: serde_json::from_str(&headers).unwrap_or_default(),
        body: row.get(6)?,
        received_at: DateTime::parse_from_rfc3339(&received_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_default(),
    })
}

/// Helper to parse a response row into ResponseRecord
fn response_from_row(row: &Row) -> tokio_rusqlite::rusqlite::Result<ResponseRecord> {
    let id: String = row.get(0)?;
    let request_id: String = row.get(1)?;
    let headers: String = row.get(3)?;
    let duration_ms: i64 = row.get(6)?;

    Ok(ResponseRecord {
        id: Uuid::parse_str(&id).unwrap_or_default(),
        request_id: Uuid::parse_str(&request_id).unwrap_or_default(),
        status_code: row.get(2)?,
        headers: serde_json::from_str(&headers).unwrap_or_default(),
        body: row.get(4)?,
        lua_script: row.get(5)?,
        duration_ms: duration_ms as u64,
        error: row.get(7)?,
    })
}

/// Get requests with optional filtering and pagination
pub async fn get_requests(conn: &Connection, query: RequestQuery) -> Result<Vec<RequestRecord>> {
    conn.call(
        move |conn| -> tokio_rusqlite::rusqlite::Result<Vec<RequestRecord>> {
            let mut sql = String::from(
                "SELECT id, domain, method, path, query_string, headers, body, received_at
             FROM requests WHERE 1=1",
            );
            let mut params_vec: Vec<Box<dyn tokio_rusqlite::rusqlite::ToSql>> = Vec::new();

            if let Some(ref domain) = query.domain {
                sql.push_str(" AND domain = ?");
                params_vec.push(Box::new(domain.clone()));
            }
            if let Some(ref method) = query.method {
                sql.push_str(" AND method = ?");
                params_vec.push(Box::new(method.clone()));
            }
            if let Some(ref path) = query.path {
                sql.push_str(" AND path LIKE ?");
                params_vec.push(Box::new(format!("%{path}%")));
            }
            if let Some(since) = query.since {
                sql.push_str(" AND received_at >= ?");
                params_vec.push(Box::new(since.to_rfc3339()));
            }

            sql.push_str(" ORDER BY received_at DESC LIMIT ? OFFSET ?");
            params_vec.push(Box::new(query.limit()));
            params_vec.push(Box::new(query.offset()));

            let params_refs: Vec<&dyn tokio_rusqlite::rusqlite::ToSql> =
                params_vec.iter().map(|p| p.as_ref()).collect();

            let mut stmt = conn.prepare(&sql)?;
            let requests = stmt
                .query_map(params_refs.as_slice(), request_from_row)?
                .collect::<tokio_rusqlite::rusqlite::Result<Vec<_>>>()?;

            Ok(requests)
        },
    )
    .await
    .map_err(Into::into)
}

/// Get a single request by ID
pub async fn get_request_by_id(conn: &Connection, id: Uuid) -> Result<Option<RequestRecord>> {
    conn.call(
        move |conn| -> tokio_rusqlite::rusqlite::Result<Option<RequestRecord>> {
            let mut stmt = conn.prepare(
                "SELECT id, domain, method, path, query_string, headers, body, received_at
             FROM requests WHERE id = ?1",
            )?;
            let result = stmt
                .query_row(params![id.to_string()], request_from_row)
                .optional()?;
            Ok(result)
        },
    )
    .await
    .map_err(Into::into)
}

/// Get the response for a given request ID
pub async fn get_response_by_request_id(
    conn: &Connection,
    request_id: Uuid,
) -> Result<Option<ResponseRecord>> {
    conn.call(
        move |conn| -> tokio_rusqlite::rusqlite::Result<Option<ResponseRecord>> {
            let mut stmt = conn.prepare(
                "SELECT id, request_id, status_code, headers, body, lua_script, duration_ms, error
             FROM responses WHERE request_id = ?1",
            )?;
            let result = stmt
                .query_row(params![request_id.to_string()], response_from_row)
                .optional()?;
            Ok(result)
        },
    )
    .await
    .map_err(Into::into)
}

/// Delete all stored requests (responses cascade)
pub async fn delete_all_requests(conn: &Connection) -> Result<u64> {
    conn.call(|conn| -> tokio_rusqlite::rusqlite::Result<u64> {
        let count = conn.execute("DELETE FROM requests", [])?;
        Ok(count as u64)
    })
    .await
    .map_err(Into::into)
}

/// Delete requests older than retention period
pub async fn cleanup_old_requests(conn: &Connection, retention_days: u32) -> Result<u64> {
    let cutoff = Utc::now() - Duration::days(retention_days as i64);
    cleanup_requests_before(conn, cutoff).await
}

/// Delete requests received before the given cutoff time
pub async fn cleanup_requests_before(conn: &Connection, cutoff: DateTime<Utc>) -> Result<u64> {
    conn.call(move |conn| -> tokio_rusqlite::rusqlite::Result<u64> {
        let count = conn.execute(
            "DELETE FROM requests WHERE received_at < ?1",
            params![cutoff.to_rfc3339()],
        )?;
        Ok(count as u64)
    })
    .await
    .map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper to create an in-memory database for testing
    async fn test_db() -> Connection {
        init_database(":memory:", 2).await.unwrap()
    }

    /// Helper to create a test request record
    fn test_request(domain: &str, method: &str, path: &str) -> RequestRecord {
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
    fn test_response(request_id: Uuid) -> ResponseRecord {
        ResponseRecord {
            id: Uuid::new_v4(),
            request_id,
            status_code: 200,
            headers: json!({"content-type": "application/json"}),
            body: Some(b"test body".to_vec()),
            lua_script: "test/init.lua".to_string(),
            duration_ms: 42,
            error: None,
        }
    }

    #[tokio::test]
    async fn test_init_database_creates_tables() {
        let conn = test_db().await;

        // Verify tables exist by querying them
        let result = conn
            .call(|conn| -> tokio_rusqlite::rusqlite::Result<i64> {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('requests', 'responses')",
                    [],
                    |row| row.get(0),
                )?;
                Ok(count)
            })
            .await
            .unwrap();

        assert_eq!(result, 2, "Both requests and responses tables should exist");
    }

    #[tokio::test]
    async fn test_init_database_creates_indexes() {
        let conn = test_db().await;

        let result = conn
            .call(|conn| -> tokio_rusqlite::rusqlite::Result<i64> {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'",
                    [],
                    |row| row.get(0),
                )?;
                Ok(count)
            })
            .await
            .unwrap();

        assert_eq!(result, 3, "Should have 3 indexes");
    }

    #[tokio::test]
    async fn test_init_database_enables_foreign_keys() {
        let conn = test_db().await;

        let result = conn
            .call(|conn| -> tokio_rusqlite::rusqlite::Result<i64> {
                let enabled: i64 = conn.query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
                Ok(enabled)
            })
            .await
            .unwrap();

        assert_eq!(result, 1, "Foreign keys should be enabled");
    }

    #[tokio::test]
    async fn test_init_database_sets_wal_mode() {
        let conn = test_db().await;

        let result = conn
            .call(|conn| -> tokio_rusqlite::rusqlite::Result<String> {
                let mode: String = conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
                Ok(mode)
            })
            .await
            .unwrap();

        // In-memory databases use "memory" journal mode
        assert!(result == "wal" || result == "memory");
    }

    #[tokio::test]
    async fn test_store_request_success() {
        let conn = test_db().await;
        let request = test_request("example.com", "GET", "/api/test");

        store_request(&conn, &request).await.unwrap();

        // Verify it was stored
        let stored = get_request_by_id(&conn, request.id).await.unwrap();
        assert!(stored.is_some());
        let stored = stored.unwrap();
        assert_eq!(stored.domain, "example.com");
        assert_eq!(stored.method, "GET");
        assert_eq!(stored.path, "/api/test");
    }

    #[tokio::test]
    async fn test_store_request_with_body() {
        let conn = test_db().await;
        let mut request = test_request("example.com", "POST", "/api/data");
        request.body = Some(b"test request body".to_vec());

        store_request(&conn, &request).await.unwrap();

        let stored = get_request_by_id(&conn, request.id).await.unwrap().unwrap();
        assert_eq!(stored.body, Some(b"test request body".to_vec()));
    }

    #[tokio::test]
    async fn test_store_request_with_query_string() {
        let conn = test_db().await;
        let mut request = test_request("example.com", "GET", "/search");
        request.query_string = Some("q=test&page=1".to_string());

        store_request(&conn, &request).await.unwrap();

        let stored = get_request_by_id(&conn, request.id).await.unwrap().unwrap();
        assert_eq!(stored.query_string, Some("q=test&page=1".to_string()));
    }

    #[tokio::test]
    async fn test_store_response_success() {
        let conn = test_db().await;
        let request = test_request("example.com", "GET", "/test");
        store_request(&conn, &request).await.unwrap();

        let response = test_response(request.id);
        store_response(&conn, &response).await.unwrap();

        let stored = get_response_by_request_id(&conn, request.id).await.unwrap();
        assert!(stored.is_some());
        let stored = stored.unwrap();
        assert_eq!(stored.status_code, 200);
        assert_eq!(stored.duration_ms, 42);
    }

    #[tokio::test]
    async fn test_store_response_with_error() {
        let conn = test_db().await;
        let request = test_request("example.com", "GET", "/error");
        store_request(&conn, &request).await.unwrap();

        let mut response = test_response(request.id);
        response.status_code = 500;
        response.error = Some("Lua error: something went wrong".to_string());
        store_response(&conn, &response).await.unwrap();

        let stored = get_response_by_request_id(&conn, request.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.status_code, 500);
        assert_eq!(
            stored.error,
            Some("Lua error: something went wrong".to_string())
        );
    }

    #[tokio::test]
    async fn test_get_requests_all() {
        let conn = test_db().await;

        // Store multiple requests
        for i in 0..5 {
            let request = test_request("example.com", "GET", &format!("/path/{}", i));
            store_request(&conn, &request).await.unwrap();
        }

        let requests = get_requests(&conn, RequestQuery::default()).await.unwrap();
        assert_eq!(requests.len(), 5);
    }

    #[tokio::test]
    async fn test_get_requests_filter_by_domain() {
        let conn = test_db().await;

        store_request(&conn, &test_request("example.com", "GET", "/a"))
            .await
            .unwrap();
        store_request(&conn, &test_request("example.com", "GET", "/b"))
            .await
            .unwrap();
        store_request(&conn, &test_request("other.com", "GET", "/c"))
            .await
            .unwrap();

        let query = RequestQuery {
            domain: Some("example.com".to_string()),
            ..Default::default()
        };
        let requests = get_requests(&conn, query).await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r.domain == "example.com"));
    }

    #[tokio::test]
    async fn test_get_requests_filter_by_method() {
        let conn = test_db().await;

        store_request(&conn, &test_request("example.com", "GET", "/a"))
            .await
            .unwrap();
        store_request(&conn, &test_request("example.com", "POST", "/b"))
            .await
            .unwrap();
        store_request(&conn, &test_request("example.com", "GET", "/c"))
            .await
            .unwrap();

        let query = RequestQuery {
            method: Some("POST".to_string()),
            ..Default::default()
        };
        let requests = get_requests(&conn, query).await.unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].method, "POST");
    }

    #[tokio::test]
    async fn test_get_requests_filter_by_path() {
        let conn = test_db().await;

        store_request(&conn, &test_request("example.com", "GET", "/api/users"))
            .await
            .unwrap();
        store_request(&conn, &test_request("example.com", "GET", "/api/posts"))
            .await
            .unwrap();
        store_request(&conn, &test_request("example.com", "GET", "/health"))
            .await
            .unwrap();

        let query = RequestQuery {
            path: Some("api".to_string()),
            ..Default::default()
        };
        let requests = get_requests(&conn, query).await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests.iter().all(|r| r.path.contains("api")));
    }

    #[tokio::test]
    async fn test_get_requests_pagination_limit() {
        let conn = test_db().await;

        for i in 0..10 {
            let request = test_request("example.com", "GET", &format!("/path/{}", i));
            store_request(&conn, &request).await.unwrap();
        }

        let query = RequestQuery {
            limit: Some(3),
            ..Default::default()
        };
        let requests = get_requests(&conn, query).await.unwrap();
        assert_eq!(requests.len(), 3);
    }

    #[tokio::test]
    async fn test_get_requests_pagination_offset() {
        let conn = test_db().await;

        for i in 0..10 {
            let request = test_request("example.com", "GET", &format!("/path/{}", i));
            store_request(&conn, &request).await.unwrap();
            // Small delay to ensure ordering
            tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        }

        let query = RequestQuery {
            limit: Some(3),
            offset: Some(3),
            ..Default::default()
        };
        let requests = get_requests(&conn, query).await.unwrap();
        assert_eq!(requests.len(), 3);
    }

    #[tokio::test]
    async fn test_get_requests_limit_capped_at_500() {
        let query = RequestQuery {
            limit: Some(1000),
            ..Default::default()
        };
        assert_eq!(query.limit(), 500);
    }

    #[tokio::test]
    async fn test_get_requests_default_limit() {
        let query = RequestQuery::default();
        assert_eq!(query.limit(), 50);
    }

    #[tokio::test]
    async fn test_get_request_by_id_exists() {
        let conn = test_db().await;
        let request = test_request("example.com", "GET", "/test");
        store_request(&conn, &request).await.unwrap();

        let result = get_request_by_id(&conn, request.id).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().id, request.id);
    }

    #[tokio::test]
    async fn test_get_request_by_id_not_found() {
        let conn = test_db().await;
        let random_id = Uuid::new_v4();

        let result = get_request_by_id(&conn, random_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_get_response_by_request_id_exists() {
        let conn = test_db().await;
        let request = test_request("example.com", "GET", "/test");
        store_request(&conn, &request).await.unwrap();

        let response = test_response(request.id);
        store_response(&conn, &response).await.unwrap();

        let result = get_response_by_request_id(&conn, request.id).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().request_id, request.id);
    }

    #[tokio::test]
    async fn test_get_response_by_request_id_not_found() {
        let conn = test_db().await;
        let random_id = Uuid::new_v4();

        let result = get_response_by_request_id(&conn, random_id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_delete_all_requests_success() {
        let conn = test_db().await;

        for i in 0..5 {
            let request = test_request("example.com", "GET", &format!("/path/{}", i));
            store_request(&conn, &request).await.unwrap();
        }

        let deleted = delete_all_requests(&conn).await.unwrap();
        assert_eq!(deleted, 5);

        let remaining = get_requests(&conn, RequestQuery::default()).await.unwrap();
        assert!(remaining.is_empty());
    }

    #[tokio::test]
    async fn test_delete_all_requests_cascades_to_responses() {
        let conn = test_db().await;

        let request = test_request("example.com", "GET", "/test");
        store_request(&conn, &request).await.unwrap();

        let response = test_response(request.id);
        store_response(&conn, &response).await.unwrap();

        delete_all_requests(&conn).await.unwrap();

        // Response should also be deleted due to cascade
        let result = get_response_by_request_id(&conn, request.id).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_cleanup_old_requests_preserves_recent() {
        let conn = test_db().await;

        // Store a recent request
        let request = test_request("example.com", "GET", "/recent");
        store_request(&conn, &request).await.unwrap();

        // Cleanup with 7 day retention
        let deleted = cleanup_old_requests(&conn, 7).await.unwrap();
        assert_eq!(deleted, 0);

        // Request should still exist
        let result = get_request_by_id(&conn, request.id).await.unwrap();
        assert!(result.is_some());
    }

    #[tokio::test]
    async fn test_cleanup_old_requests_deletes_old() {
        let conn = test_db().await;

        // Store request and manually update its timestamp to be old
        let request = test_request("example.com", "GET", "/old");
        store_request(&conn, &request).await.unwrap();

        // Update the received_at to be 10 days ago
        let old_time = Utc::now() - Duration::days(10);
        conn.call(move |conn| -> tokio_rusqlite::rusqlite::Result<()> {
            conn.execute(
                "UPDATE requests SET received_at = ?1 WHERE id = ?2",
                params![old_time.to_rfc3339(), request.id.to_string()],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        // Cleanup with 7 day retention
        let deleted = cleanup_old_requests(&conn, 7).await.unwrap();
        assert_eq!(deleted, 1);
    }

    #[tokio::test]
    async fn test_request_headers_stored_as_json() {
        let conn = test_db().await;
        let mut request = test_request("example.com", "GET", "/test");
        request.headers = json!({
            "content-type": "application/json",
            "authorization": "Bearer token123",
            "x-custom": "value"
        });

        store_request(&conn, &request).await.unwrap();

        let stored = get_request_by_id(&conn, request.id).await.unwrap().unwrap();
        assert_eq!(stored.headers["content-type"], "application/json");
        assert_eq!(stored.headers["authorization"], "Bearer token123");
        assert_eq!(stored.headers["x-custom"], "value");
    }

    #[tokio::test]
    async fn test_cleanup_requests_before_cutoff() {
        let conn = test_db().await;

        // Store two requests
        let old_request = test_request("example.com", "GET", "/old");
        let new_request = test_request("example.com", "GET", "/new");
        store_request(&conn, &old_request).await.unwrap();
        store_request(&conn, &new_request).await.unwrap();

        // Backdate the old request to 10 days ago
        let old_time = Utc::now() - Duration::days(10);
        conn.call(move |conn| -> tokio_rusqlite::rusqlite::Result<()> {
            conn.execute(
                "UPDATE requests SET received_at = ?1 WHERE id = ?2",
                params![old_time.to_rfc3339(), old_request.id.to_string()],
            )?;
            Ok(())
        })
        .await
        .unwrap();

        // Delete everything older than 5 days ago
        let cutoff = Utc::now() - Duration::days(5);
        let deleted = cleanup_requests_before(&conn, cutoff).await.unwrap();
        assert_eq!(deleted, 1);

        // New request should still exist
        let remaining = get_requests(&conn, RequestQuery::default()).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].path, "/new");
    }
}
