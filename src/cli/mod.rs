// SPDX-FileCopyrightText: 2026 Will Dixon
//
// SPDX-License-Identifier: AGPL-3.0-only

//! Command-line interface definitions and implementations.

mod check;
mod init;
mod new;
mod serve;

use clap::{Parser, Subcommand};
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

pub use check::CheckArgs;
pub use init::InitArgs;
pub use new::NewArgs;
pub use serve::ServeArgs;

#[derive(Parser)]
#[command(name = "mockserver")]
#[command(about = "A Lua-powered mock server for API development")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Increase logging verbosity (can be repeated: -v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count, global = true)]
    pub verbose: u8,

    /// Suppress non-error output
    #[arg(short, long, global = true)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the mock server
    Serve(ServeArgs),

    /// Initialize a new mocks directory with example files
    Init(InitArgs),

    /// Create a new domain mock folder
    New(NewArgs),

    /// Validate domains and check for errors
    Check(CheckArgs),
}

impl Cli {
    /// Initialize tracing based on verbosity flags.
    pub fn init_tracing(&self) {
        let level = if self.quiet {
            "error"
        } else {
            match self.verbose {
                0 => "info",
                1 => "debug",
                _ => "trace",
            }
        };

        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(format!("mockserver={level},info")));

        tracing_subscriber::registry()
            .with(filter)
            .with(fmt::layer())
            .init();
    }

    /// Execute the CLI command.
    pub async fn run(self) -> anyhow::Result<()> {
        self.init_tracing();

        match self.command {
            Commands::Serve(args) => args.run().await,
            Commands::Init(args) => args.run(),
            Commands::New(args) => args.run(),
            Commands::Check(args) => args.run(),
        }
    }
}
