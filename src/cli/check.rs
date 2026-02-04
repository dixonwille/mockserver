// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! The `check` command - validates Lua syntax and reports errors.

use std::fs;
use std::path::PathBuf;

use clap::Args;
use serde::Serialize;

use crate::lua::{DomainEntry, list_domains, register_stub_modules};

#[derive(Args)]
pub struct CheckArgs {
    /// Mocks directory
    #[arg(short, long, default_value = "./.mockserver/mocks", env = "MOCKSERVER_DIR")]
    dir: PathBuf,

    /// Check only a specific domain
    domain: Option<String>,

    /// Output as JSON
    #[arg(long)]
    json: bool,

    /// Brief output (no detailed error messages)
    #[arg(short, long)]
    brief: bool,
}

#[derive(Serialize)]
struct CheckResult {
    name: String,
    has_init: bool,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

enum CheckStatus {
    Ok,
    MissingInit,
    SyntaxError(String),
    MissingHandle,
}

impl CheckArgs {
    pub fn run(self) -> anyhow::Result<()> {
        if !self.dir.exists() {
            anyhow::bail!("Mocks directory not found: {:?}", self.dir);
        }

        let entries = if let Some(ref domain) = self.domain {
            // Check specific domain
            let path = self.dir.join(domain);
            if !path.exists() {
                anyhow::bail!("Domain not found: {}", domain);
            }
            let init_path = path.join("init.lua");
            vec![DomainEntry {
                name: domain.clone(),
                path,
                has_init: init_path.exists(),
                init_path,
            }]
        } else {
            list_domains(&self.dir)?
        };

        if entries.is_empty() {
            if self.json {
                println!("[]");
            } else {
                println!("No domains found in {}", self.dir.display());
                println!();
                println!("Create one with: mockserver new <domain>");
            }
            return Ok(());
        }

        let mut results = Vec::new();
        let mut has_errors = false;

        for entry in entries {
            let status = check_domain(&entry);
            if !matches!(status, CheckStatus::Ok) {
                has_errors = true;
            }

            let (status_str, error) = match &status {
                CheckStatus::Ok => ("ok", None),
                CheckStatus::MissingInit => ("missing_init", None),
                CheckStatus::MissingHandle => ("missing_handle", None),
                CheckStatus::SyntaxError(e) => ("error", Some(e.clone())),
            };

            results.push(CheckResult {
                name: entry.name.clone(),
                has_init: entry.has_init,
                status: status_str,
                error,
            });
        }

        if self.json {
            println!("{}", serde_json::to_string_pretty(&results)?);
        } else if self.brief {
            self.print_brief(&results);
        } else {
            self.print_detailed(&results);
        }

        if has_errors {
            std::process::exit(1);
        }

        Ok(())
    }

    fn print_brief(&self, results: &[CheckResult]) {
        println!("Domains in {}:", self.dir.display());
        println!();
        for result in results {
            let status = match result.status {
                "ok" => "✓ ok",
                "missing_init" => "⚠ missing init.lua",
                "missing_handle" => "✗ missing handle()",
                _ => "✗ error",
            };
            println!("  {} [{}]", result.name, status);
        }
        println!();
        println!("Total: {} domain(s)", results.len());
    }

    fn print_detailed(&self, results: &[CheckResult]) {
        for result in results {
            match result.status {
                "ok" => {
                    println!("✓ {}", result.name);
                }
                "missing_init" => {
                    println!("✗ {} - missing init.lua", result.name);
                }
                "missing_handle" => {
                    println!("✗ {} - missing handle() function", result.name);
                }
                _ => {
                    println!("✗ {} - syntax error:", result.name);
                    if let Some(ref err) = result.error {
                        for line in err.lines() {
                            println!("    {}", line);
                        }
                    }
                }
            }
        }

        let ok_count = results.iter().filter(|r| r.status == "ok").count();
        let error_count = results.len() - ok_count;

        println!();
        if error_count > 0 {
            println!(
                "Checked {} domain(s): {} ok, {} with errors",
                results.len(),
                ok_count,
                error_count
            );
        } else {
            println!("Checked {} domain(s): all ok", results.len());
        }
    }
}

fn check_domain(entry: &DomainEntry) -> CheckStatus {
    if !entry.has_init {
        return CheckStatus::MissingInit;
    }

    let content = match fs::read_to_string(&entry.init_path) {
        Ok(c) => c,
        Err(e) => {
            return CheckStatus::SyntaxError(format!("Failed to read file: {}", e));
        }
    };

    let lua = mlua::Lua::new();
    let chunk_name = format!("{}/init.lua", entry.name);

    // Register stub modules so require() works during checking
    if let Err(e) = register_stub_modules(&lua) {
        return CheckStatus::SyntaxError(format!("Failed to setup Lua: {}", e));
    }

    // Try to compile the Lua code
    if let Err(e) = lua.load(&content).set_name(&chunk_name).into_function() {
        return CheckStatus::SyntaxError(format_lua_error(&e));
    }

    // Execute the code to define functions
    if let Err(e) = lua.load(&content).set_name(&chunk_name).exec() {
        return CheckStatus::SyntaxError(format_lua_error(&e));
    }

    // Check for handle function
    let globals = lua.globals();
    match globals.get::<mlua::Function>("handle") {
        Ok(_) => CheckStatus::Ok,
        Err(_) => CheckStatus::MissingHandle,
    }
}

fn format_lua_error(err: &mlua::Error) -> String {
    match err {
        mlua::Error::SyntaxError { message, .. } => message.clone(),
        mlua::Error::RuntimeError(msg) => msg.clone(),
        other => other.to_string(),
    }
}
