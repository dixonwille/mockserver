// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Async delay/sleep module using tokio

use mlua::{Lua, Result as LuaResult};

use super::preload_module;

pub fn register(lua: &Lua) -> LuaResult<()> {
    let delay = lua.create_table()?;

    // delay.sleep(ms) - async sleep using tokio
    delay.set(
        "sleep",
        lua.create_async_function(|_, ms: u64| async move {
            tokio::time::sleep(std::time::Duration::from_millis(ms)).await;
            Ok(())
        })?,
    )?;

    preload_module(lua, "delay", delay)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delay_module_exists() {
        let lua = Lua::new();
        register(&lua).unwrap();

        // Verify the module loads
        lua.load(r#"local delay = require("delay")"#)
            .exec()
            .unwrap();
    }

    #[tokio::test]
    async fn test_delay_sleep_works() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let start = std::time::Instant::now();
        lua.load(r#"require("delay").sleep(10)"#)
            .exec_async()
            .await
            .unwrap();
        let elapsed = start.elapsed();

        // Should have slept at least 10ms (allowing for some overhead)
        assert!(elapsed.as_millis() >= 10);
    }
}
