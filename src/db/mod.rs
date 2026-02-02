// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Database module for request/response storage
//!
//! Uses SQLite with WAL mode via tokio-rusqlite for async access.
//! Schema is managed via migrations in the `migrations` module.

mod migrations;
mod models;
mod queries;

pub use migrations::{current_version, migrate, migrations};
pub use models::*;
pub use queries::*;
