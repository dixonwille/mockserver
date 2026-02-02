// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! JSON encode/decode module via serde_json

use mlua::{Lua, Result as LuaResult, Value};

use super::{json_to_lua, lua_to_json, preload_module};

pub fn register(lua: &Lua) -> LuaResult<()> {
    let json = lua.create_table()?;

    // json.encode(value) -> string
    json.set(
        "encode",
        lua.create_function(|lua, value: Value| {
            let json_value = lua_to_json(lua, value)?;
            serde_json::to_string(&json_value)
                .map_err(|e| mlua::Error::external(format!("JSON encode error: {e}")))
        })?,
    )?;

    // json.decode(string) -> value
    json.set(
        "decode",
        lua.create_function(|lua, s: String| {
            let json_value: serde_json::Value = serde_json::from_str(&s)
                .map_err(|e| mlua::Error::external(format!("JSON decode error: {e}")))?;
            json_to_lua(lua, json_value)
        })?,
    )?;

    preload_module(lua, "json", json)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_roundtrip() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(
                r#"
                local json = require("json")
                local data = {name = "Alice", age = 30}
                return json.encode(data)
            "#,
            )
            .eval()
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["name"], "Alice");
        assert_eq!(parsed["age"], 30);
    }

    #[test]
    fn test_json_encode_null() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(r#"return require("json").encode(nil)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "null");
    }

    #[test]
    fn test_json_encode_boolean() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(r#"return require("json").encode(true)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "true");

        let result: String = lua
            .load(r#"return require("json").encode(false)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "false");
    }

    #[test]
    fn test_json_encode_numbers() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(r#"return require("json").encode(42)"#)
            .eval()
            .unwrap();
        assert_eq!(result, "42");

        let result: String = lua
            .load(r#"return require("json").encode(3.14)"#)
            .eval()
            .unwrap();
        assert!(result.starts_with("3.14"));
    }

    #[test]
    fn test_json_encode_string() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(r#"return require("json").encode("hello")"#)
            .eval()
            .unwrap();
        assert_eq!(result, "\"hello\"");
    }

    #[test]
    fn test_json_encode_array() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(r#"return require("json").encode({1, 2, 3})"#)
            .eval()
            .unwrap();
        assert_eq!(result, "[1,2,3]");
    }

    #[test]
    fn test_json_encode_nested() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(
                r#"
                local json = require("json")
                return json.encode({
                    user = {name = "Bob", tags = {"admin", "user"}}
                })
            "#,
            )
            .eval()
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["user"]["name"], "Bob");
        assert_eq!(parsed["user"]["tags"][0], "admin");
    }

    #[test]
    fn test_json_decode_object() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(
                r#"
                local json = require("json")
                local data = json.decode('{"name":"Alice","age":30}')
                return data.name
            "#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn test_json_decode_array() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: i64 = lua
            .load(
                r#"
                local json = require("json")
                local data = json.decode('[1, 2, 3]')
                return data[2]
            "#,
            )
            .eval()
            .unwrap();
        assert_eq!(result, 2);
    }

    #[test]
    fn test_json_decode_invalid() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: Result<String, _> = lua
            .load(r#"return require("json").decode("not valid json")"#)
            .eval();
        assert!(result.is_err());
    }

    #[test]
    fn test_lua_to_json_empty_table() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(r#"return require("json").encode({})"#)
            .eval()
            .unwrap();

        // Empty table could be [] or {}
        assert!(result == "[]" || result == "{}");
    }

    #[test]
    fn test_lua_to_json_mixed_table() {
        let lua = Lua::new();
        register(&lua).unwrap();

        // Table with only string keys should be object
        let result: String = lua
            .load(r#"return require("json").encode({a = 1, b = 2})"#)
            .eval()
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert!(parsed.is_object());
    }

    #[test]
    fn test_json_to_lua_deeply_nested() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: i64 = lua
            .load(
                r#"
                local json = require("json")
                local data = json.decode('{"a":{"b":{"c":{"d":42}}}}')
                return data.a.b.c.d
            "#,
            )
            .eval()
            .unwrap();

        assert_eq!(result, 42);
    }
}
