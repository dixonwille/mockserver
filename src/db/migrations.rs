// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Database migrations
//!
//! Uses rusqlite_migration to manage schema versions. The schema version is
//! tracked using SQLite's built-in `user_version` pragma.
//!
//! # Adding new migrations
//!
//! 1. Add a new `M::up(...)` entry to the MIGRATIONS vector
//! 2. Never modify existing migrations - always add new ones
//! 3. Run `cargo test db::migrations` to validate

use rusqlite_migration::{M, Migrations};
use tokio_rusqlite::rusqlite::Connection;

/// All database migrations, applied in order.
///
/// Migration 1 is the "zero-state" baseline containing the complete initial schema.
/// Future migrations should be added incrementally below it.
///
/// # Important
///
/// Never modify existing migrations. Always add new ones.
pub fn migrations() -> Migrations<'static> {
    Migrations::new(vec![
        // ============================================================
        // V1: Zero-state baseline schema
        // ============================================================
        // This migration creates the complete initial schema. New databases
        // only need to run this single migration. When adding new migrations,
        // append them below - do not modify this baseline.
        M::up(
            "-- Requests table: stores incoming HTTP requests
            CREATE TABLE requests (
                id TEXT PRIMARY KEY,
                domain TEXT NOT NULL,
                method TEXT NOT NULL,
                path TEXT NOT NULL,
                query_string TEXT,
                headers TEXT NOT NULL,
                body BLOB,
                received_at TEXT NOT NULL
            );

            -- Index for filtering by domain with time ordering
            CREATE INDEX idx_requests_domain_received
                ON requests(domain, received_at DESC);

            -- Index for global time-based queries
            CREATE INDEX idx_requests_received
                ON requests(received_at DESC);

            -- Responses table: stores generated responses for each request
            CREATE TABLE responses (
                id TEXT PRIMARY KEY,
                request_id TEXT NOT NULL,
                status_code INTEGER NOT NULL,
                headers TEXT NOT NULL,
                body BLOB,
                lua_script TEXT,
                duration_ms INTEGER,
                error TEXT,
                FOREIGN KEY (request_id) REFERENCES requests(id) ON DELETE CASCADE
            );

            -- Index for looking up response by request
            CREATE INDEX idx_responses_request_id
                ON responses(request_id);",
        ),
        // ============================================================
        // Future migrations go here
        // ============================================================
        // Example:
        // M::up("ALTER TABLE requests ADD COLUMN client_ip TEXT"),
    ])
}

/// Apply all pending migrations to bring the database to the current version.
pub fn migrate(conn: &mut Connection) -> Result<(), rusqlite_migration::Error> {
    migrations().to_latest(conn)
}

/// Get the current schema version from the database.
pub fn current_version(conn: &Connection) -> Result<usize, rusqlite_migration::Error> {
    match migrations().current_version(conn)? {
        rusqlite_migration::SchemaVersion::NoneSet => Ok(0),
        rusqlite_migration::SchemaVersion::Inside(v) => Ok(v.into()),
        rusqlite_migration::SchemaVersion::Outside(_) => Ok(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_valid() {
        // Validates all migrations can be applied to an empty database
        migrations().validate().unwrap();
    }

    #[test]
    fn can_apply_all_migrations() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();

        // Verify we're at version 1
        let version = current_version(&conn).unwrap();
        assert_eq!(version, 1);
    }

    #[test]
    fn creates_requests_table() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();

        // Verify requests table exists with expected columns
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(requests)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(columns.contains(&"id".to_string()));
        assert!(columns.contains(&"domain".to_string()));
        assert!(columns.contains(&"method".to_string()));
        assert!(columns.contains(&"path".to_string()));
        assert!(columns.contains(&"headers".to_string()));
        assert!(columns.contains(&"received_at".to_string()));
    }

    #[test]
    fn creates_responses_table() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();

        // Verify responses table exists with expected columns
        let columns: Vec<String> = conn
            .prepare("PRAGMA table_info(responses)")
            .unwrap()
            .query_map([], |row| row.get(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(columns.contains(&"id".to_string()));
        assert!(columns.contains(&"request_id".to_string()));
        assert!(columns.contains(&"status_code".to_string()));
        assert!(columns.contains(&"headers".to_string()));
        assert!(columns.contains(&"lua_script".to_string()));
    }

    #[test]
    fn creates_indexes() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();

        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        assert_eq!(count, 3, "Should have 3 indexes");
    }

    #[test]
    fn idempotent_migration() {
        let mut conn = Connection::open_in_memory().unwrap();

        // Apply migrations twice - should not error
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();

        let version = current_version(&conn).unwrap();
        assert_eq!(version, 1);
    }
}
