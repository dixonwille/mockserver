// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Host-provided Lua modules
//!
//! Provides json, log, delay, uuid, time, fs, and state modules to Lua scripts.

mod delay;
mod fs;
mod json;
mod log;
mod state;
mod time;
mod uuid;

use mlua::{Lua, Result as LuaResult, Table, Value};
use std::path::Path;

pub use state::DomainState;

/// Names of all host-provided modules.
/// Used by the check command to register stub modules.
pub const MODULE_NAMES: &[&str] = &["json", "log", "delay", "uuid", "time", "fs", "state"];

/// Register stub modules that return empty tables.
/// Used for syntax checking without full module initialization.
pub fn register_stub_modules(lua: &Lua) -> LuaResult<()> {
    let preload: Table = lua.globals().get::<Table>("package")?.get("preload")?;

    for &name in MODULE_NAMES {
        let stub = lua.create_function(|lua, _: ()| lua.create_table())?;
        preload.set(name, stub)?;
    }

    Ok(())
}

/// Register all host-provided modules with a Lua state
pub fn register_modules(
    lua: &Lua,
    domain: &str,
    domain_dir: &Path,
    state: DomainState,
) -> LuaResult<()> {
    json::register(lua)?;
    log::register(lua, domain)?;
    delay::register(lua)?;
    uuid::register(lua)?;
    time::register(lua)?;
    fs::register(lua, domain_dir)?;
    state::register(lua, state)?;
    Ok(())
}

/// Add a module to package.loaded so it can be required
pub(crate) fn preload_module(lua: &Lua, name: &str, module: Table) -> LuaResult<()> {
    let package: Table = lua.globals().get("package")?;
    let loaded: Table = package.get("loaded")?;
    loaded.set(name, module)?;
    Ok(())
}

/// Convert a Lua value to a serde_json Value
pub(crate) fn lua_to_json(_lua: &Lua, value: Value) -> LuaResult<serde_json::Value> {
    match value {
        Value::Nil => Ok(serde_json::Value::Null),
        Value::Boolean(b) => Ok(serde_json::Value::Bool(b)),
        Value::Integer(i) => Ok(serde_json::Value::Number(i.into())),
        Value::Number(n) => serde_json::Number::from_f64(n)
            .map(serde_json::Value::Number)
            .ok_or_else(|| mlua::Error::external("Invalid number (NaN or Infinity)")),
        Value::String(s) => Ok(serde_json::Value::String(s.to_str()?.to_string())),
        Value::Table(t) => {
            // Check if it's an array (sequential integer keys starting at 1)
            let len = t.clone().pairs::<Value, Value>().count();
            let array_len = t.raw_len();

            if array_len > 0 && array_len == len {
                // Likely an array
                let mut arr = Vec::with_capacity(array_len);
                for i in 1..=array_len {
                    let v: Value = t.get(i)?;
                    arr.push(lua_to_json(_lua, v)?);
                }
                Ok(serde_json::Value::Array(arr))
            } else {
                // Object
                let mut map = serde_json::Map::new();
                for pair in t.pairs::<String, Value>() {
                    let (k, v) = pair?;
                    map.insert(k, lua_to_json(_lua, v)?);
                }
                Ok(serde_json::Value::Object(map))
            }
        }
        _ => Err(mlua::Error::external(format!(
            "Cannot convert {:?} to JSON",
            value.type_name()
        ))),
    }
}

/// Convert a serde_json Value to a Lua value
pub(crate) fn json_to_lua(lua: &Lua, value: serde_json::Value) -> LuaResult<Value> {
    match value {
        serde_json::Value::Null => Ok(Value::Nil),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Err(mlua::Error::external("Invalid number"))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(lua.create_string(&s)?)),
        serde_json::Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.into_iter().enumerate() {
                table.set(i + 1, json_to_lua(lua, v)?)?;
            }
            Ok(Value::Table(table))
        }
        serde_json::Value::Object(obj) => {
            let table = lua.create_table()?;
            for (k, v) in obj {
                table.set(k, json_to_lua(lua, v)?)?;
            }
            Ok(Value::Table(table))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};
    use tempfile::TempDir;

    #[test]
    fn test_register_modules_all_available() {
        let temp_dir = TempDir::new().unwrap();
        let lua = Lua::new();
        let state = Arc::new(RwLock::new(HashMap::new()));

        register_modules(&lua, "test.domain", temp_dir.path(), state).unwrap();

        // Verify all modules are available
        lua.load(
            r#"
            local json = require("json")
            local log = require("log")
            local delay = require("delay")
            local uuid = require("uuid")
            local time = require("time")
            local fs = require("fs")
            local state = require("state")
        "#,
        )
        .exec()
        .unwrap();
    }
}
