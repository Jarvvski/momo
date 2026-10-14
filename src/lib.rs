//! momo's library: a pure versioning domain, the adapters that reach the
//! outside world, and the CLI that wires them together.
//!
//! The [`domain`] and [`adapters`] modules are public so the versioning model
//! can be reused; the command-line surface is private and reached only through
//! [`run`].
//!
//! momo is Unix-only: its process handling leads each child in its own process
//! group so a timeout stops everything the child forked, and there is no
//! equivalent off Unix. The build fails clearly there rather than shipping a
//! half-supported fallback.

#[cfg(not(unix))]
compile_error!("momo targets Unix only (Linux and macOS); Windows is not supported");

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
