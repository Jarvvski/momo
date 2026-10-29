//! momo's crate root: a pure versioning domain, the adapters that reach the
//! outside world, and the CLI that wires them together.
//!
//! The library is an implementation detail of the `momo` binary, not a
//! supported API: only [`run`] is exported, and the domain and adapter modules
//! stay crate-private. See ADR 0005, *The library is not a public API*.
//!
//! momo is Unix-only: its process handling leads each child in its own process
//! group so a timeout stops everything the child forked, and there is no
//! equivalent off Unix. The build fails clearly there rather than shipping a
//! half-supported fallback.

#[cfg(not(unix))]
compile_error!("momo targets Unix only (Linux and macOS); Windows is not supported");

// The internal API is broader than the CLI: the domain models the whole
// versioning vocabulary and the adapters implement every port, while the tests
// exercise more of it than any one verb reaches. In a non-test build those
// parts look dead, so dead-code analysis is off there — `mise run lint` also
// compiles the test build, where it still runs.
#[cfg_attr(not(test), allow(dead_code))]
mod adapters;
#[cfg_attr(not(test), allow(dead_code))]
mod domain;

mod cli;

use std::process::ExitCode;

use clap::Parser;

use crate::cli::Format;
use crate::cli::diagnostic::diagnose;

/// Parse the process arguments and run the requested verb.
pub fn run() -> ExitCode {
    let cli = cli::Cli::parse();
    match cli::commands::run(&cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            // A failed run still honours `--format` and its `--json`
            // shorthand: a machine consumer gets the structured error on
            // stdout, a human gets it on stderr. The exit code is non-zero
            // either way, and `--output` is never written.
            let diagnostic = diagnose(&err);
            match cli.output_format() {
                Format::Human => eprintln!("{}", diagnostic.human()),
                Format::Json => print!("{}", diagnostic.json()),
            }
            ExitCode::FAILURE
        }
    }
}
