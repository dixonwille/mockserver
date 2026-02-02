// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Mockserver - A Lua-powered mock server for API development
//!
//! This crate provides a configurable mock server that:
//! - Routes HTTP requests based on domain
//! - Executes Lua scripts to generate responses
//! - Stores all requests/responses for inspection
//! - Supports hot-reloading of Lua files

pub mod api;
pub mod cli;
pub mod config;
pub mod db;
pub mod error;
pub mod lua;
pub mod server;
pub mod templates;
pub mod watcher;

pub use config::ServerConfig;
pub use error::{Error, Result};
