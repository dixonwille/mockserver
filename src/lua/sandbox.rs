// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Lua sandboxing for security
//!
//! Removes dangerous modules and configures package.path for domain isolation.

use mlua::{Lua, Result as LuaResult, Table, Value};
use std::path::Path;

/// Apply sandboxing to a Lua state by removing dangerous modules
pub fn sandbox_lua(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    // Save safe os functions before removing os module
    let os: Table = globals.get("os")?;
    let os_date: mlua::Function = os.get("date")?;
    let os_time: mlua::Function = os.get("time")?;
    let os_difftime: mlua::Function = os.get("difftime")?;

    // Remove dangerous modules completely
    globals.set("os", Value::Nil)?;
    globals.set("io", Value::Nil)?;
    globals.set("loadfile", Value::Nil)?;
    globals.set("dofile", Value::Nil)?;
    globals.set("load", Value::Nil)?; // Prevent loading bytecode

    // Remove debug module (can be used to escape sandbox)
    globals.set("debug", Value::Nil)?;

    // Remove package.loadlib (C module loading)
    let package: Table = globals.get("package")?;
    package.set("loadlib", Value::Nil)?;

    // Create a safe os module with only date, time, and difftime
    let safe_os = lua.create_table()?;
    safe_os.set("date", os_date)?;
    safe_os.set("time", os_time)?;
    safe_os.set("difftime", os_difftime)?;
    globals.set("os", safe_os)?;

    Ok(())
}

/// Configure package.path to only allow requires from the domain folder
pub fn configure_package_path(lua: &Lua, domain_dir: &Path) -> LuaResult<()> {
    let package: Table = lua.globals().get("package")?;

    // Only allow requires from the domain folder
    // ./?.lua      - for require("helpers")
    // ./?/init.lua - for require("routes") loading routes/init.lua
    let path = format!("{0}/?.lua;{0}/?/init.lua", domain_dir.display());
    package.set("path", path)?;

    // Disable C module loading entirely
    package.set("cpath", "")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sandbox_removes_dangerous_modules() {
        let lua = Lua::new();
        sandbox_lua(&lua).unwrap();

        let globals = lua.globals();

        // io should be removed
        assert!(globals.get::<Value>("io").unwrap().is_nil());

        // loadfile, dofile, and load should be removed
        assert!(globals.get::<Value>("loadfile").unwrap().is_nil());
        assert!(globals.get::<Value>("dofile").unwrap().is_nil());
        assert!(globals.get::<Value>("load").unwrap().is_nil());

        // debug should be removed
        assert!(globals.get::<Value>("debug").unwrap().is_nil());

        // os.date, os.time, and os.difftime should still work
        let os: Table = globals.get("os").unwrap();
        assert!(os.get::<mlua::Function>("date").is_ok());
        assert!(os.get::<mlua::Function>("time").is_ok());
        assert!(os.get::<mlua::Function>("difftime").is_ok());

        // But os.execute should not exist
        assert!(os.get::<Value>("execute").unwrap().is_nil());
    }

    #[test]
    fn test_package_path_configuration() {
        let lua = Lua::new();
        let domain_dir = Path::new("/mocks/api.example.com");

        configure_package_path(&lua, domain_dir).unwrap();

        let package: Table = lua.globals().get("package").unwrap();
        let path: String = package.get("path").unwrap();
        let cpath: String = package.get("cpath").unwrap();

        assert!(path.contains("/mocks/api.example.com/?.lua"));
        assert!(path.contains("/mocks/api.example.com/?/init.lua"));
        assert!(cpath.is_empty());
    }
}
