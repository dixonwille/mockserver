// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Embedded template files for mockserver initialization and domain creation.

// Type definition files for _types/ directory
pub const TYPES_LUA: &str = include_str!("templates/_types/types.lua");
pub const JSON_LUA: &str = include_str!("templates/_types/json.lua");
pub const LOG_LUA: &str = include_str!("templates/_types/log.lua");
pub const DELAY_LUA: &str = include_str!("templates/_types/delay.lua");
pub const STATE_LUA: &str = include_str!("templates/_types/state.lua");
pub const UUID_LUA: &str = include_str!("templates/_types/uuid.lua");
pub const TIME_LUA: &str = include_str!("templates/_types/time.lua");
pub const FS_LUA: &str = include_str!("templates/_types/fs.lua");

// IDE configuration
pub const LUARC_JSON: &str = include_str!("templates/luarc.json");

// Default fallback handler
pub const DEFAULT_INIT_LUA: &str = include_str!("templates/default_init.lua");

// Domain templates
pub const TEMPLATE_BASIC: &str = include_str!("templates/basic.lua");
pub const TEMPLATE_REST: &str = include_str!("templates/rest.lua");
pub const TEMPLATE_GRAPHQL: &str = include_str!("templates/graphql.lua");
