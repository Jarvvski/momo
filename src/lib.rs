//! momo's library: a pure versioning domain, the adapters that reach the
//! outside world, and the CLI that wires them together.
//!
//! The [`domain`] and [`adapters`] modules are public so the versioning model
//! can be reused; the command-line surface is private and reached only through
//! [`run`].

pub mod adapters;
pub mod domain;

mod cli;

use std::process::ExitCode;

use clap::Parser;

/// Parse the process arguments and run the requested verb.
pub fn run() -> ExitCode {
    let cli = cli::Cli::parse();
    match cli::commands::run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
