// SPDX-FileCopyrightText: 2026 mockserver contributors
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

    #[test]
    fn test_register_stub_modules() {
        let lua = Lua::new();
        register_stub_modules(&lua).unwrap();

        // Each stub module should return an empty table
        for &name in MODULE_NAMES {
            let result: Table = lua
                .load(format!("return require('{name}')"))
                .eval()
                .unwrap();
            assert_eq!(
                result.clone().pairs::<Value, Value>().count(),
                0,
                "Stub module '{name}' should be an empty table"
            );
        }
    }

    // ==================== lua_to_json tests ====================

    #[test]
    fn test_lua_to_json_nil() {
        let lua = Lua::new();
        let result = lua_to_json(&lua, Value::Nil).unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[test]
    fn test_lua_to_json_boolean() {
        let lua = Lua::new();
        assert_eq!(
            lua_to_json(&lua, Value::Boolean(true)).unwrap(),
            serde_json::Value::Bool(true)
        );
        assert_eq!(
            lua_to_json(&lua, Value::Boolean(false)).unwrap(),
            serde_json::Value::Bool(false)
        );
    }

    #[test]
    fn test_lua_to_json_integer() {
        let lua = Lua::new();
        let result = lua_to_json(&lua, Value::Integer(42)).unwrap();
        assert_eq!(result, serde_json::json!(42));
    }

    #[test]
    fn test_lua_to_json_float() {
        let lua = Lua::new();
        let result = lua_to_json(&lua, Value::Number(2.75)).unwrap();
        assert_eq!(result, serde_json::json!(2.75));
    }

    #[test]
    fn test_lua_to_json_string() {
        let lua = Lua::new();
        let lua_str = lua.create_string("hello").unwrap();
        let result = lua_to_json(&lua, Value::String(lua_str)).unwrap();
        assert_eq!(result, serde_json::json!("hello"));
    }

    #[test]
    fn test_lua_to_json_nan_errors() {
        let lua = Lua::new();
        let result = lua_to_json(&lua, Value::Number(f64::NAN));
        assert!(result.is_err());
    }

    #[test]
    fn test_lua_to_json_infinity_errors() {
        let lua = Lua::new();
        let result = lua_to_json(&lua, Value::Number(f64::INFINITY));
        assert!(result.is_err());
    }

    #[test]
    fn test_lua_to_json_sequential_array() {
        let lua = Lua::new();
        let table = lua.create_table().unwrap();
        table.set(1, "a").unwrap();
        table.set(2, "b").unwrap();
        table.set(3, "c").unwrap();

        let result = lua_to_json(&lua, Value::Table(table)).unwrap();
        assert_eq!(result, serde_json::json!(["a", "b", "c"]));
    }

    #[test]
    fn test_lua_to_json_object() {
        let lua = Lua::new();
        let table = lua.create_table().unwrap();
        table.set("name", "test").unwrap();
        table.set("value", 42).unwrap();

        let result = lua_to_json(&lua, Value::Table(table)).unwrap();
        assert_eq!(result["name"], serde_json::json!("test"));
        assert_eq!(result["value"], serde_json::json!(42));
    }

    #[test]
    fn test_lua_to_json_mixed_table() {
        let lua = Lua::new();
        // Table with both integer and string keys — has 3 pairs total but raw_len=2
        // So it will be treated as an object
        let table = lua.create_table().unwrap();
        table.set(1, "first").unwrap();
        table.set(2, "second").unwrap();
        table.set("extra", "value").unwrap();

        let result = lua_to_json(&lua, Value::Table(table)).unwrap();
        assert!(result.is_object());
    }

    #[test]
    fn test_lua_to_json_empty_table() {
        let lua = Lua::new();
        let table = lua.create_table().unwrap();
        let result = lua_to_json(&lua, Value::Table(table)).unwrap();
        // Empty table: raw_len is 0 and pair count is 0, so it goes to the object branch
        assert_eq!(result, serde_json::json!({}));
    }

    #[test]
    fn test_lua_to_json_unsupported_type_errors() {
        let lua = Lua::new();
        let func = lua.create_function(|_, ()| Ok(())).unwrap();
        let result = lua_to_json(&lua, Value::Function(func));
        assert!(result.is_err());
    }

    // ==================== json_to_lua tests ====================

    #[test]
    fn test_json_to_lua_null() {
        let lua = Lua::new();
        let result = json_to_lua(&lua, serde_json::Value::Null).unwrap();
        assert!(matches!(result, Value::Nil));
    }

    #[test]
    fn test_json_to_lua_bool() {
        let lua = Lua::new();
        let result = json_to_lua(&lua, serde_json::json!(true)).unwrap();
        assert!(matches!(result, Value::Boolean(true)));
    }

    #[test]
    fn test_json_to_lua_integer() {
        let lua = Lua::new();
        let result = json_to_lua(&lua, serde_json::json!(42)).unwrap();
        assert!(matches!(result, Value::Integer(42)));
    }

    #[test]
    fn test_json_to_lua_float() {
        let lua = Lua::new();
        let result = json_to_lua(&lua, serde_json::json!(2.75)).unwrap();
        match result {
            Value::Number(n) => assert!((n - 2.75).abs() < f64::EPSILON),
            _ => panic!("expected Number, got {:?}", result),
        }
    }

    #[test]
    fn test_json_to_lua_string() {
        let lua = Lua::new();
        let result = json_to_lua(&lua, serde_json::json!("hello")).unwrap();
        match result {
            Value::String(s) => assert_eq!(s.to_str().unwrap(), "hello"),
            _ => panic!("expected String"),
        }
    }

    #[test]
    fn test_json_to_lua_array() {
        let lua = Lua::new();
        let result = json_to_lua(&lua, serde_json::json!([1, 2, 3])).unwrap();
        match result {
            Value::Table(t) => {
                assert_eq!(t.get::<i64>(1).unwrap(), 1);
                assert_eq!(t.get::<i64>(2).unwrap(), 2);
                assert_eq!(t.get::<i64>(3).unwrap(), 3);
            }
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn test_json_to_lua_object() {
        let lua = Lua::new();
        let result = json_to_lua(&lua, serde_json::json!({"key": "value"})).unwrap();
        match result {
            Value::Table(t) => {
                assert_eq!(t.get::<String>("key").unwrap(), "value");
            }
            _ => panic!("expected Table"),
        }
    }

    #[test]
    fn test_json_to_lua_roundtrip() {
        let lua = Lua::new();
        let original = serde_json::json!({
            "name": "test",
            "count": 42,
            "active": true,
            "tags": ["a", "b"],
            "nested": {"key": "val"}
        });

        let lua_val = json_to_lua(&lua, original.clone()).unwrap();
        let back = lua_to_json(&lua, lua_val).unwrap();

        assert_eq!(original, back);
    }
}
