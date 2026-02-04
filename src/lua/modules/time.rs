// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Time utilities module (now, now_ms, iso8601, format)

use chrono::{TimeZone, Utc};
use mlua::{Lua, Result as LuaResult};

use super::preload_module;

pub fn register(lua: &Lua) -> LuaResult<()> {
    let time = lua.create_table()?;

    // time.now() -> unix timestamp (seconds)
    time.set(
        "now",
        lua.create_function(|_, ()| Ok(Utc::now().timestamp()))?,
    )?;

    // time.now_ms() -> unix timestamp (milliseconds)
    time.set(
        "now_ms",
        lua.create_function(|_, ()| Ok(Utc::now().timestamp_millis()))?,
    )?;

    // time.iso8601() -> ISO 8601 formatted string
    time.set(
        "iso8601",
        lua.create_function(|_, ()| Ok(Utc::now().to_rfc3339()))?,
    )?;

    // time.format(fmt, timestamp?) -> formatted string
    time.set(
        "format",
        lua.create_function(|_, (fmt, ts): (String, Option<i64>)| {
            let dt = match ts {
                Some(timestamp) => Utc
                    .timestamp_opt(timestamp, 0)
                    .single()
                    .ok_or_else(|| mlua::Error::external("Invalid timestamp"))?,
                None => Utc::now(),
            };
            Ok(dt.format(&fmt).to_string())
        })?,
    )?;

    preload_module(lua, "time", time)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_module() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: i64 = lua
            .load(
                r#"
                local time = require("time")
                return time.now()
            "#,
            )
            .eval()
            .unwrap();

        // Should be a reasonable timestamp
        assert!(result > 1700000000);
    }

    #[test]
    fn test_time_now_ms() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: i64 = lua
            .load(r#"return require("time").now_ms()"#)
            .eval()
            .unwrap();

        // Should be in milliseconds (much larger than seconds)
        assert!(result > 1700000000000i64);
    }

    #[test]
    fn test_time_iso8601() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(r#"return require("time").iso8601()"#)
            .eval()
            .unwrap();

        // Should be valid RFC3339 format
        assert!(result.contains("T"));
        assert!(result.contains(":"));
        assert!(chrono::DateTime::parse_from_rfc3339(&result).is_ok());
    }

    #[test]
    fn test_time_format_custom() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(r#"return require("time").format("%Y-%m-%d")"#)
            .eval()
            .unwrap();

        // Should match YYYY-MM-DD format
        assert!(result.len() == 10);
        assert!(result.contains("-"));
    }

    #[test]
    fn test_time_format_with_timestamp() {
        let lua = Lua::new();
        register(&lua).unwrap();

        let result: String = lua
            .load(r#"return require("time").format("%Y-%m-%d", 0)"#)
            .eval()
            .unwrap();

        // Unix epoch should be 1970-01-01
        assert_eq!(result, "1970-01-01");
    }

    #[test]
    fn test_time_format_specific_timestamp() {
        let lua = Lua::new();
        register(&lua).unwrap();

        // 1704067200 = 2024-01-01 00:00:00 UTC
        let result: String = lua
            .load(r#"return require("time").format("%Y-%m-%d %H:%M:%S", 1704067200)"#)
            .eval()
            .unwrap();

        assert_eq!(result, "2024-01-01 00:00:00");
    }
}
