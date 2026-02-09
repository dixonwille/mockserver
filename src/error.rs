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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_config_constructor() {
        let err = Error::config("bad config");
        assert!(matches!(err, Error::Config(ref s) if s == "bad config"));
    }

    #[test]
    fn test_error_invalid_request_constructor() {
        let err = Error::invalid_request("missing header");
        assert!(matches!(err, Error::InvalidRequest(ref s) if s == "missing header"));
    }

    #[test]
    fn test_display_config() {
        let err = Error::Config("test".to_string());
        assert_eq!(err.to_string(), "Configuration error: test");
    }

    #[test]
    fn test_display_mocks_dir_not_found() {
        let err = Error::MocksDirNotFound("/missing".to_string());
        assert_eq!(err.to_string(), "Mocks directory not found: /missing");
    }

    #[test]
    fn test_display_script_not_found() {
        let err = Error::ScriptNotFound("example.com".to_string());
        assert_eq!(
            err.to_string(),
            "Lua script not found for domain: example.com"
        );
    }

    #[test]
    fn test_display_missing_handle_function() {
        let err = Error::MissingHandleFunction("example.com".to_string());
        assert_eq!(
            err.to_string(),
            "Missing handle() function in domain: example.com"
        );
    }

    #[test]
    fn test_display_lua_syntax() {
        let err = Error::LuaSyntax {
            domain: "example.com".to_string(),
            file: "init.lua".to_string(),
            error: "unexpected symbol".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Lua syntax error in example.com/init.lua: unexpected symbol"
        );
    }

    #[test]
    fn test_display_script_timeout() {
        let err = Error::ScriptTimeout("example.com".to_string());
        assert_eq!(
            err.to_string(),
            "Script execution timeout for domain: example.com"
        );
    }

    #[test]
    fn test_display_pool_exhausted() {
        let err = Error::PoolExhausted("example.com".to_string());
        assert_eq!(
            err.to_string(),
            "Lua runtime pool exhausted for domain: example.com"
        );
    }

    #[test]
    fn test_display_database() {
        let err = Error::Database("connection failed".to_string());
        assert_eq!(err.to_string(), "Database error: connection failed");
    }

    #[test]
    fn test_display_invalid_request() {
        let err = Error::InvalidRequest("bad uuid".to_string());
        assert_eq!(err.to_string(), "Invalid request: bad uuid");
    }

    #[test]
    fn test_display_no_domain() {
        let err = Error::NoDomain;
        assert_eq!(err.to_string(), "No domain in request headers");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err: Error = io_err.into();
        assert!(matches!(err, Error::Io(_)));
        assert!(err.to_string().contains("file missing"));
    }

    #[test]
    fn test_from_serde_json_error() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let err: Error = json_err.into();
        assert!(matches!(err, Error::Json(_)));
    }

    #[test]
    fn test_from_notify_error() {
        let notify_err = notify::Error::generic("watcher failed");
        let err: Error = notify_err.into();
        assert!(matches!(err, Error::Watcher(_)));
        assert!(err.to_string().contains("watcher failed"));
    }

    #[test]
    fn test_from_tokio_rusqlite_connection_closed() {
        let tr_err = tokio_rusqlite::Error::<tokio_rusqlite::rusqlite::Error>::ConnectionClosed;
        let err: Error = tr_err.into();
        assert!(matches!(err, Error::Database(ref s) if s == "Connection closed"));
    }

    #[tokio::test]
    async fn test_from_tokio_rusqlite_close_error() {
        // Create a tokio_rusqlite Connection for the Close variant
        let conn = tokio_rusqlite::Connection::open_in_memory().await.unwrap();
        let rusqlite_err = tokio_rusqlite::rusqlite::Error::QueryReturnedNoRows;
        let tr_err = tokio_rusqlite::Error::Close((conn, rusqlite_err));
        let err: Error = tr_err.into();
        assert!(matches!(err, Error::Sqlite(_)));
    }

    #[test]
    fn test_from_tokio_rusqlite_error() {
        let rusqlite_err = tokio_rusqlite::rusqlite::Error::QueryReturnedNoRows;
        let tr_err = tokio_rusqlite::Error::Error(rusqlite_err);
        let err: Error = tr_err.into();
        assert!(matches!(err, Error::Sqlite(_)));
    }
}
