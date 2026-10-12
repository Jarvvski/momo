//! momo entry point.

use std::process::ExitCode;

fn main() -> ExitCode {
    println!("momo {}", momo::VERSION);
    ExitCode::SUCCESS
}
