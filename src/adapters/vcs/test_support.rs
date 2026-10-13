//! A scripted [`CommandRunner`] for unit tests: no process is ever spawned.
//!
//! Each reply names the exact command it answers. A command the script does not
//! cover panics, so an adapter that runs the wrong thing — a lightweight tag
//! where an annotated one belongs, or a create after a failed existence check —
//! fails the test rather than passing quietly.

use std::path::Path;

use crate::domain::vcs::VcsError;

use super::process::{CommandRunner, ProcessOutput};

struct Reply {
    program: String,
    args: Vec<String>,
    output: ProcessOutput,
}

/// A [`CommandRunner`] that answers from a fixed script.
pub(crate) struct FakeRunner {
    replies: Vec<Reply>,
}

impl FakeRunner {
    /// An empty script.
    pub(crate) fn new() -> Self {
        Self {
            replies: Vec::new(),
        }
    }

    /// Script `program args` to succeed with `stdout`.
    pub(crate) fn ok(self, program: &str, args: &[&str], stdout: &str) -> Self {
        self.script(
            program,
            args,
            ProcessOutput {
                success: true,
                stdout: stdout.to_owned(),
                stderr: String::new(),
            },
        )
    }

    /// Script `program args` to fail with `stderr`.
    pub(crate) fn fail(self, program: &str, args: &[&str], stderr: &str) -> Self {
        self.script(
            program,
            args,
            ProcessOutput {
                success: false,
                stdout: String::new(),
                stderr: stderr.to_owned(),
            },
        )
    }

    fn script(mut self, program: &str, args: &[&str], output: ProcessOutput) -> Self {
        self.replies.push(Reply {
            program: program.to_owned(),
            args: args.iter().map(|arg| (*arg).to_owned()).collect(),
            output,
        });
        self
    }
}

impl CommandRunner for FakeRunner {
    fn run(&self, program: &str, args: &[&str], _cwd: &Path) -> Result<ProcessOutput, VcsError> {
        let wanted: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
        Ok(self
            .replies
            .iter()
            .find(|reply| reply.program == program && reply.args == wanted)
            .map(|reply| reply.output.clone())
            .unwrap_or_else(|| panic!("unexpected command: {program} {wanted:?}")))
    }
}
