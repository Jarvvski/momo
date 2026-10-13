//! momo entry point.

use std::process::ExitCode;

fn main() -> ExitCode {
    println!("momo {}", env!("CARGO_PKG_VERSION"));
    ExitCode::SUCCESS
}
