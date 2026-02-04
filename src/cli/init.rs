// SPDX-FileCopyrightText: 2026 mockserver contributors
//
// SPDX-License-Identifier: AGPL-3.0-only

//! The `init` command - initializes a new mocks directory.

use std::fs;
use std::path::PathBuf;

use clap::Args;

use crate::templates;

#[derive(Args)]
pub struct InitArgs {
    /// Directory to initialize
    #[arg(default_value = "./.mockserver/mocks")]
    path: PathBuf,

    /// Update _types/ type definitions and .luarc.json (preserves _default/)
    #[arg(short, long)]
    force: bool,
}

impl InitArgs {
    pub fn run(self) -> anyhow::Result<()> {
        let path = &self.path;

        let types_dir = path.join("_types");
        let default_dir = path.join("_default");
        let already_initialized = types_dir.exists();

        if already_initialized && !self.force {
            anyhow::bail!(
                "Mocks directory already initialized at {:?}. Use --force to update type definitions.",
                path
            );
        }

        // Create base directories
        fs::create_dir_all(path)?;
        fs::create_dir_all(&default_dir)?;

        // Remove and recreate _types/ to ensure clean type definitions
        if types_dir.exists() {
            fs::remove_dir_all(&types_dir)?;
        }
        fs::create_dir_all(&types_dir)?;

        // Generate type definition files
        write_type_definitions(&types_dir)?;

        // Generate .luarc.json
        write_luarc_json(path)?;

        // Generate _default/init.lua only if it doesn't exist
        let default_init = default_dir.join("init.lua");
        let created_default = if !default_init.exists() {
            fs::write(&default_init, templates::DEFAULT_INIT_LUA)?;
            true
        } else {
            false
        };

        if already_initialized {
            println!("Updated mocks directory at {}", path.display());
            println!("  Refreshed _types/ (type definitions)");
            println!("  Refreshed .luarc.json (IDE configuration)");
            if created_default {
                println!("  Created _default/init.lua (fallback handler)");
            } else {
                println!("  Preserved _default/init.lua");
            }
        } else {
            println!("Initialized mocks directory at {}", path.display());
            println!("  Created _types/ (type definitions)");
            println!("  Created .luarc.json (IDE configuration)");
            println!("  Created _default/init.lua (fallback handler)");
            println!();
            println!("Next steps:");
            println!("  1. Create a domain: mockserver new api.example.com");
            println!("  2. Start the server: mockserver serve");
        }

        Ok(())
    }
}

fn write_type_definitions(dir: &std::path::Path) -> anyhow::Result<()> {
    fs::write(dir.join("types.lua"), templates::TYPES_LUA)?;
    fs::write(dir.join("json.lua"), templates::JSON_LUA)?;
    fs::write(dir.join("log.lua"), templates::LOG_LUA)?;
    fs::write(dir.join("delay.lua"), templates::DELAY_LUA)?;
    fs::write(dir.join("state.lua"), templates::STATE_LUA)?;
    fs::write(dir.join("uuid.lua"), templates::UUID_LUA)?;
    fs::write(dir.join("time.lua"), templates::TIME_LUA)?;
    fs::write(dir.join("fs.lua"), templates::FS_LUA)?;
    Ok(())
}

fn write_luarc_json(mocks_dir: &std::path::Path) -> anyhow::Result<()> {
    fs::write(mocks_dir.join(".luarc.json"), templates::LUARC_JSON)?;
    Ok(())
}
