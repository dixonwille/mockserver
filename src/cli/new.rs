// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! The `new` command - creates a new domain mock folder.

use std::fs;
use std::path::PathBuf;

use clap::Args;

use crate::templates;

#[derive(Args)]
pub struct NewArgs {
    /// Domain name (e.g., api.example.com)
    domain: String,

    /// Mocks directory
    #[arg(
        short,
        long,
        default_value = "./.mockserver/mocks",
        env = "MOCKSERVER_DIR"
    )]
    dir: PathBuf,

    /// Template type: basic, rest, graphql
    #[arg(short, long, default_value = "basic")]
    template: String,

    /// Overwrite existing folder
    #[arg(short, long)]
    force: bool,
}

impl NewArgs {
    pub fn run(self) -> anyhow::Result<()> {
        // Validate domain name
        if self.domain.starts_with('.') || self.domain.contains("..") || self.domain.contains('/') {
            anyhow::bail!("Invalid domain name: {}", self.domain);
        }

        let domain_dir = self.dir.join(&self.domain);

        if domain_dir.exists() && !self.force {
            anyhow::bail!(
                "Domain folder already exists: {:?}. Use --force to overwrite.",
                domain_dir
            );
        }

        fs::create_dir_all(&domain_dir)?;

        // Select template
        let init_content = match self.template.as_str() {
            "basic" => templates::TEMPLATE_BASIC.replace("{{DOMAIN}}", &self.domain),
            "rest" => templates::TEMPLATE_REST.replace("{{DOMAIN}}", &self.domain),
            "graphql" => templates::TEMPLATE_GRAPHQL.replace("{{DOMAIN}}", &self.domain),
            _ => anyhow::bail!(
                "Unknown template: {}. Use: basic, rest, graphql",
                self.template
            ),
        };

        fs::write(domain_dir.join("init.lua"), init_content)?;

        println!("Created domain mock: {}", self.domain);
        println!("  Directory: {}", domain_dir.display());
        println!("  Template: {}", self.template);
        println!();
        println!(
            "Edit {}/init.lua to customize the mock behavior.",
            self.domain
        );

        Ok(())
    }
}
