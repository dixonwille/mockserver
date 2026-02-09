// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Logging module via tracing

use mlua::{Lua, Result as LuaResult};
use tracing::{debug, error, info, warn};

use super::preload_module;

pub fn register(lua: &Lua, domain: &str) -> LuaResult<()> {
    let log = lua.create_table()?;
    let domain_owned = domain.to_string();

    let domain_clone = domain_owned.clone();
    log.set(
        "debug",
        lua.create_function(move |_, msg: String| {
            debug!(domain = %domain_clone, "{}", msg);
            Ok(())
        })?,
    )?;

    let domain_clone = domain_owned.clone();
    log.set(
        "info",
        lua.create_function(move |_, msg: String| {
            info!(domain = %domain_clone, "{}", msg);
            Ok(())
        })?,
    )?;

    let domain_clone = domain_owned.clone();
    log.set(
        "warn",
        lua.create_function(move |_, msg: String| {
            warn!(domain = %domain_clone, "{}", msg);
            Ok(())
        })?,
    )?;

    let domain_clone = domain_owned;
    log.set(
        "error",
        lua.create_function(move |_, msg: String| {
            error!(domain = %domain_clone, "{}", msg);
            Ok(())
        })?,
    )?;

    preload_module(lua, "log", log)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_module_functions_exist() {
        let lua = Lua::new();
        register(&lua, "test.domain").unwrap();

        // Just verify the functions can be called without error
        lua.load(
            r#"
            local log = require("log")
            log.debug("debug message")
            log.info("info message")
            log.warn("warn message")
            log.error("error message")
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn test_log_debug_callable() {
        let lua = Lua::new();
        register(&lua, "test.domain").unwrap();
        lua.load(r#"require("log").debug("test debug")"#)
            .exec()
            .unwrap();
    }

    #[test]
    fn test_log_info_callable() {
        let lua = Lua::new();
        register(&lua, "test.domain").unwrap();
        lua.load(r#"require("log").info("test info")"#)
            .exec()
            .unwrap();
    }

    #[test]
    fn test_log_warn_callable() {
        let lua = Lua::new();
        register(&lua, "test.domain").unwrap();
        lua.load(r#"require("log").warn("test warn")"#)
            .exec()
            .unwrap();
    }

    #[test]
    fn test_log_error_callable() {
        let lua = Lua::new();
        register(&lua, "test.domain").unwrap();
        lua.load(r#"require("log").error("test error")"#)
            .exec()
            .unwrap();
    }
}
