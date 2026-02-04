// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! UUID generation module

use mlua::{Lua, Result as LuaResult};

use super::preload_module;

pub fn register(lua: &Lua) -> LuaResult<()> {
    let uuid_mod = lua.create_table()?;

    // uuid.v4() -> string
    uuid_mod.set(
        "v4",
        lua.create_function(|_, ()| Ok(uuid::Uuid::new_v4().to_string()))?,
    )?;

    preload_module(lua, "uuid", uuid_mod)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_generation() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(
                r#"
                local uuid = require("uuid")
                return uuid.v4()
            "#,
            )
            .eval()
            .unwrap();

        // Should be valid UUID format
        assert!(uuid::Uuid::parse_str(&result).is_ok());
    }

    #[test]
    fn test_uuid_uniqueness() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: (String, String) = lua
            .load(
                r#"
                local uuid = require("uuid")
                return uuid.v4(), uuid.v4()
            "#,
            )
            .eval()
            .unwrap();

        assert_ne!(result.0, result.1);
    }

    #[test]
    fn test_uuid_v4_format() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua.load(r#"return require("uuid").v4()"#).eval().unwrap();

        // UUID v4 format: xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx
        assert_eq!(result.len(), 36);
        assert_eq!(&result[14..15], "4"); // Version 4
    }
}
