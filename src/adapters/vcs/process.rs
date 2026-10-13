//! The process boundary: the adapters shell out through [`CommandRunner`] so
//! unit tests can inject a scripted fake instead of spawning git or jj.

use std::path::Path;
use std::process::Command;

use crate::domain::vcs::VcsError;

/// The result of running one external command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    /// Whether the command exited with status zero.
    pub success: bool,
    /// Captured standard output, decoded lossily as UTF-8.
    pub stdout: String,
    /// Captured standard error, decoded lossily as UTF-8.
    pub stderr: String,
}

/// Runs external commands.
///
/// Implemented by [`SystemRunner`] in production and by a fake in tests; keeping
/// every spawn behind this trait is what lets the adapters be tested without a
/// process.
pub trait CommandRunner {
    /// Run `program` with `args` in `cwd`.
    fn run(&self, program: &str, args: &[&str], cwd: &Path) -> Result<ProcessOutput, VcsError>;
}

/// The real runner: spawns a child process and captures its output.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemRunner;

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str], cwd: &Path) -> Result<ProcessOutput, VcsError> {
        let output = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .output()
            .map_err(|source| VcsError::Spawn {
                program: program.to_owned(),
                source,
            })?;

        Ok(ProcessOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

/// Turn a non-zero exit into [`VcsError::CommandFailed`], leaving a successful
/// run untouched.
pub(crate) fn require_success(
    program: &str,
    args: &[&str],
    output: ProcessOutput,
) -> Result<ProcessOutput, VcsError> {
    if output.success {
        Ok(output)
    } else {
        Err(VcsError::CommandFailed {
            program: program.to_owned(),
            args: args.join(" "),
            stderr: output.stderr.trim().to_owned(),
        })
    }
}
