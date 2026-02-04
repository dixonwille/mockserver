// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

use thiserror::Error;

/// Main error type for the mockserver application
#[derive(Error, Debug)]
pub enum Error {
    // Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Mocks directory not found: {0}")]
    MocksDirNotFound(String),

    // Lua errors
    #[error("Lua error: {0}")]
    Lua(#[from] mlua::Error),

    #[error("Lua script not found for domain: {0}")]
    ScriptNotFound(String),

    #[error("Missing handle() function in domain: {0}")]
    MissingHandleFunction(String),

    #[error("Lua syntax error in {domain}/{file}: {error}")]
    LuaSyntax {
        domain: String,
        file: String,
        error: String,
    },

    #[error("Script execution timeout for domain: {0}")]
    ScriptTimeout(String),

    #[error("Lua runtime pool exhausted for domain: {0}")]
    PoolExhausted(String),

    // Database errors
    #[error("Database error: {0}")]
    Database(String),

    #[error("SQLite error: {0}")]
    Sqlite(#[from] tokio_rusqlite::rusqlite::Error),

    // HTTP/Server errors
    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("No domain in request headers")]
    NoDomain,

    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // File watcher errors
    #[error("File watcher error: {0}")]
    Watcher(#[from] notify::Error),

    // Serialization errors
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result type alias using our Error
pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    /// Create a configuration error
    pub fn config(msg: impl Into<String>) -> Self {
        Error::Config(msg.into())
    }

    /// Create an invalid request error
    pub fn invalid_request(msg: impl Into<String>) -> Self {
        Error::InvalidRequest(msg.into())
    }
}

impl From<tokio_rusqlite::Error<tokio_rusqlite::rusqlite::Error>> for Error {
    fn from(err: tokio_rusqlite::Error<tokio_rusqlite::rusqlite::Error>) -> Self {
        match err {
            tokio_rusqlite::Error::ConnectionClosed => {
                Error::Database("Connection closed".to_string())
            }
            tokio_rusqlite::Error::Close((_, e)) => Error::Sqlite(e),
            tokio_rusqlite::Error::Error(e) => Error::Sqlite(e),
            _ => Error::Database(format!("Unknown database error: {err}")),
        }
    }
}
