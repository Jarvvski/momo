//! The process boundary: the adapters shell out through [`CommandRunner`] so
//! unit tests can inject a scripted fake instead of spawning git or jj.

use std::path::Path;
use std::time::Duration;

use crate::adapters::spawn::{self, ProcessError, ProcessOutput};
use crate::domain::config::DEFAULT_COMMAND_TIMEOUT;
use crate::domain::vcs::VcsError;

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
///
/// Every command is bounded by [`SystemRunner::timeout`]; a child that outlives
/// it is killed and reported as [`VcsError::Timeout`].
#[derive(Debug, Clone, Copy)]
pub struct SystemRunner {
    timeout: Duration,
}

impl SystemRunner {
    /// A runner that fails any command that runs longer than `timeout`.
    pub fn new(timeout: Duration) -> Self {
        Self { timeout }
    }
}

impl Default for SystemRunner {
    fn default() -> Self {
        Self::new(DEFAULT_COMMAND_TIMEOUT)
    }
}

impl CommandRunner for SystemRunner {
    fn run(&self, program: &str, args: &[&str], cwd: &Path) -> Result<ProcessOutput, VcsError> {
        spawn::run(program, args, cwd, self.timeout).map_err(VcsError::from)
    }
}

/// Turn a non-zero exit into a domain "operation failed" error, leaving a
/// successful run untouched.
///
/// The process detail (the program, its arguments, its stderr) is wrapped
/// opaquely: the domain's [`VcsError::Operation`] carries no process vocabulary,
/// and the detail is formatted at the CLI boundary.
pub(crate) fn require_success(
    program: &str,
    args: &[&str],
    output: ProcessOutput,
) -> Result<ProcessOutput, VcsError> {
    spawn::require_success(program, args, output).map_err(VcsError::from)
}

impl From<ProcessError> for VcsError {
    fn from(err: ProcessError) -> Self {
        match err.timeout() {
            Some(timeout) => Self::Timeout { timeout },
            None => Self::Operation {
                source: Box::new(err),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::config::{Config, ConfigOverrides};

    /// A scratch directory for the process tests.
    fn cwd() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("temp dir")
    }

    #[test]
    fn a_missing_program_is_an_operation_failure_that_hides_the_process_detail() {
        let dir = cwd();
        let err = SystemRunner::default()
            .run("momo-no-such-program", &[], dir.path())
            .expect_err("the program does not exist");

        let VcsError::Operation { source } = &err else {
            panic!("expected an operation failure, got {err:?}");
        };
        // The domain's own message carries no process vocabulary...
        assert!(!err.to_string().contains("momo-no-such-program"), "{err}");
        // ...while the adapter's opaque source does.
        assert!(
            source.to_string().contains("momo-no-such-program"),
            "{source}"
        );
    }

    #[test]
    fn a_command_that_outlives_the_timeout_fails_with_the_timeout_variant() {
        let dir = cwd();
        let err = SystemRunner::new(Duration::from_millis(50))
            .run("sh", &["-c", "sleep 30"], dir.path())
            .expect_err("the sleep outlives the timeout");

        assert!(
            matches!(err, VcsError::Timeout { timeout } if timeout == Duration::from_millis(50)),
            "{err:?}"
        );
    }

    #[test]
    fn the_configured_timeout_bounds_a_command() {
        // The key the user sets must reach the runner: a one-second config
        // timeout kills a command that would otherwise run for far longer.
        let file: ConfigOverrides = "command_timeout = 1\n".parse().expect("valid TOML");
        let config = Config::resolve(file, ConfigOverrides::default());
        let runner = SystemRunner::new(config.command_timeout);

        let started = std::time::Instant::now();
        let err = runner
            .run("sh", &["-c", "sleep 30"], cwd().path())
            .expect_err("the sleep outlives the configured timeout");

        assert!(matches!(err, VcsError::Timeout { .. }), "{err:?}");
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "the child was not killed: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_non_zero_exit_becomes_an_operation_failure() {
        let dir = cwd();
        let output = SystemRunner::default()
            .run("sh", &["-c", "printf fatal >&2; exit 1"], dir.path())
            .expect("the command runs");
        assert!(!output.success);

        let err = require_success("sh", &["-c", "exit 1"], output).expect_err("non-zero fails");
        assert!(matches!(err, VcsError::Operation { .. }), "{err:?}");
        assert!(err.to_string().contains("the VCS command failed"), "{err}");
    }
}
