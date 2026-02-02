// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Lua runtime module
//!
//! Manages Lua states for each domain with sandboxing and isolation.

mod manager;
mod modules;
mod sandbox;

pub use manager::{DomainEntry, LuaRequest, LuaResponse, ScriptManager, list_domains};
pub use modules::{DomainState, MODULE_NAMES, register_stub_modules};
