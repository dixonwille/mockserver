// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Per-domain key-value state storage module

use mlua::{Lua, Result as LuaResult, Value};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use super::{json_to_lua, lua_to_json, preload_module};

/// Per-domain state storage
pub type DomainState = Arc<RwLock<HashMap<String, serde_json::Value>>>;

pub fn register(lua: &Lua, state: DomainState) -> LuaResult<()> {
    let state_mod = lua.create_table()?;

    let state_clone = state.clone();
    // state.get(key) -> value or nil
    state_mod.set(
        "get",
        lua.create_function(move |lua, key: String| {
            let guard = state_clone
                .read()
                .map_err(|_| mlua::Error::external("State lock poisoned"))?;
            match guard.get(&key) {
                Some(v) => json_to_lua(lua, v.clone()),
                None => Ok(Value::Nil),
            }
        })?,
    )?;

    let state_clone = state.clone();
    // state.set(key, value)
    state_mod.set(
        "set",
        lua.create_function(move |lua, (key, value): (String, Value)| {
            let json_value = lua_to_json(lua, value)?;
            let mut guard = state_clone
                .write()
                .map_err(|_| mlua::Error::external("State lock poisoned"))?;
            guard.insert(key, json_value);
            Ok(())
        })?,
    )?;

    let state_clone = state.clone();
    // state.delete(key)
    state_mod.set(
        "delete",
        lua.create_function(move |_, key: String| {
            let mut guard = state_clone
                .write()
                .map_err(|_| mlua::Error::external("State lock poisoned"))?;
            guard.remove(&key);
            Ok(())
        })?,
    )?;

    // state.clear()
    state_mod.set(
        "clear",
        lua.create_function(move |_, ()| {
            let mut guard = state
                .write()
                .map_err(|_| mlua::Error::external("State lock poisoned"))?;
            guard.clear();
            Ok(())
        })?,
    )?;

    preload_module(lua, "state", state_mod)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_module() {
        let lua = Lua::new();
        let state = Arc::new(RwLock::new(HashMap::new()));
        register(&lua, state).unwrap();

        let result: i64 = lua
            .load(
                r#"
                local state = require("state")
                state.set("counter", 42)
                return state.get("counter")
            "#,
            )
            .eval()
            .unwrap();

        assert_eq!(result, 42);
    }

    #[test]
    fn test_state_get_nonexistent_returns_nil() {
        let lua = Lua::new();
        let state = Arc::new(RwLock::new(HashMap::new()));
        register(&lua, state).unwrap();

        let result: Value = lua
            .load(r#"return require("state").get("nonexistent")"#)
            .eval()
            .unwrap();

        assert!(matches!(result, Value::Nil));
    }

    #[test]
    fn test_state_delete() {
        let lua = Lua::new();
        let state = Arc::new(RwLock::new(HashMap::new()));
        register(&lua, state).unwrap();

        let result: Value = lua
            .load(
                r#"
                local state = require("state")
                state.set("key", "value")
                state.delete("key")
                return state.get("key")
            "#,
            )
            .eval()
            .unwrap();

        assert!(matches!(result, Value::Nil));
    }

    #[test]
    fn test_state_clear() {
        let lua = Lua::new();
        let state = Arc::new(RwLock::new(HashMap::new()));
        register(&lua, state).unwrap();

        let result: (Value, Value) = lua
            .load(
                r#"
                local state = require("state")
                state.set("a", 1)
                state.set("b", 2)
                state.clear()
                return state.get("a"), state.get("b")
            "#,
            )
            .eval()
            .unwrap();

        assert!(matches!(result.0, Value::Nil));
        assert!(matches!(result.1, Value::Nil));
    }

    #[test]
    fn test_state_set_overwrites() {
        let lua = Lua::new();
        let state = Arc::new(RwLock::new(HashMap::new()));
        register(&lua, state).unwrap();

        let result: String = lua
            .load(
                r#"
                local state = require("state")
                state.set("key", "first")
                state.set("key", "second")
                return state.get("key")
            "#,
            )
            .eval()
            .unwrap();

        assert_eq!(result, "second");
    }

    #[test]
    fn test_state_complex_values() {
        let lua = Lua::new();
        let state = Arc::new(RwLock::new(HashMap::new()));
        register(&lua, state).unwrap();

        let result: String = lua
            .load(
                r#"
                local state = require("state")
                state.set("user", {name = "Alice", age = 30})
                return state.get("user").name
            "#,
            )
            .eval()
            .unwrap();

        assert_eq!(result, "Alice");
    }
}
