//! Building and running child processes.
//!
//! Every external command momo spawns runs unattended — in CI, in the release
//! job — so it must never stop to prompt and must never hang forever. [`command`]
//! is the one place a [`Command`] is built, so the non-interactive environment is
//! applied uniformly and can be asserted without spawning a process; [`run`] is
//! the one place a child is waited on, so the timeout applies everywhere. A child
//! leads its own process group, so expiry stops what it forked as well as the
//! child itself.
//!
//! The process vocabulary a shelled-out adapter produces — the binary, its
//! arguments, its stderr — lives in [`ProcessError`], never in the domain. A
//! caller wraps it in the domain's opaque "operation failed" variant and formats
//! it at the CLI boundary.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// The environment that makes git and mise run non-interactively.
///
/// - `GIT_TERMINAL_PROMPT=0` fails instead of prompting for credentials.
/// - `GIT_EDITOR` / `GIT_SEQUENCE_EDITOR=true` never open an editor.
/// - `GIT_PAGER=cat` never opens a pager.
/// - `GIT_OPTIONAL_LOCKS=0` avoids taking locks for read-only commands.
/// - `MISE_YES=1` answers mise's trust and confirmation prompts.
const NON_INTERACTIVE: [(&str, &str); 6] = [
    ("GIT_TERMINAL_PROMPT", "0"),
    ("GIT_EDITOR", "true"),
    ("GIT_SEQUENCE_EDITOR", "true"),
    ("GIT_PAGER", "cat"),
    ("GIT_OPTIONAL_LOCKS", "0"),
    ("MISE_YES", "1"),
];

/// How often the wait loop checks whether a child has exited.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

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

/// An external command that could not be run to completion.
///
/// Adapter-owned: it carries the process detail the domain's ports deliberately
/// keep out.
#[derive(Debug, thiserror::Error)]
pub enum ProcessError {
    /// The program could not be started.
    #[error("failed to run `{program}`")]
    Spawn {
        /// The program that could not be started.
        program: String,
        /// Why the process could not be spawned.
        #[source]
        source: std::io::Error,
    },

    /// The program ran but exited non-zero.
    #[error("`{program} {args}` failed: {stderr}")]
    Failed {
        /// The program that failed.
        program: String,
        /// The arguments it was given, space-separated.
        args: String,
        /// The trimmed standard error it produced.
        stderr: String,
    },

    /// The program did not finish within the timeout.
    #[error("`{program} {args}` did not finish within {timeout:?}")]
    TimedOut {
        /// The program that timed out.
        program: String,
        /// The arguments it was given, space-separated.
        args: String,
        /// The timeout that elapsed.
        timeout: Duration,
    },
}

impl ProcessError {
    /// The timeout that elapsed, when this failure was a timeout.
    pub fn timeout(&self) -> Option<Duration> {
        match self {
            Self::TimedOut { timeout, .. } => Some(*timeout),
            Self::Spawn { .. } | Self::Failed { .. } => None,
        }
    }
}

/// A [`Command`] for `program` run with `args` in `cwd`.
///
/// The command carries the non-interactive environment, so callers spawn it
/// without thinking about prompts.
pub(crate) fn command(program: &str, args: &[&str], cwd: &Path) -> Command {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd).envs(NON_INTERACTIVE);
    command
}

/// Run `program` with `args` in `cwd`, failing after `timeout`.
///
/// The one place a child is waited on: the command runs non-interactively and
/// is killed once `timeout` elapses, so a hung git, jj or mise cannot stall the
/// run. A non-zero exit is *not* an error here — some callers expect one (a
/// tag-existence probe) and check [`ProcessOutput::success`] themselves.
///
/// The child leads its own process group, so expiry stops everything it forked
/// — a mise tool process, a `sh -c` grandchild — not just the direct child.
pub(crate) fn run(
    program: &str,
    args: &[&str],
    cwd: &Path,
    timeout: Duration,
) -> Result<ProcessOutput, ProcessError> {
    let mut built = command(program, args, cwd);
    built
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    lead_its_own_group(&mut built);
    let mut child = built.spawn().map_err(|source| ProcessError::Spawn {
        program: program.to_owned(),
        source,
    })?;

    // Drain the pipes on their own threads: a child that fills a pipe buffer
    // would otherwise block forever, which the wait loop would misread as a
    // hang and kill a healthy process.
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let stdout = thread::spawn(move || read_all(stdout));
    let stderr = thread::spawn(move || read_all(stderr));

    let deadline = Instant::now() + timeout;
    let mut wait_error = None;
    let mut status = None;
    loop {
        match child.try_wait() {
            Ok(Some(exited)) => {
                status = Some(exited);
                break;
            }
            Ok(None) if Instant::now() >= deadline => break,
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(source) => {
                wait_error = Some(source);
                break;
            }
        }
    }

    let Some(status) = status else {
        // Expiry (or a failed wait): stop the whole group and return at once.
        // The reader threads are deliberately *not* joined here — a forked
        // grandchild inherits the write end of the pipes, so waiting for them
        // would hold the call open for as long as the grandchild lives, which
        // is exactly the hang the timeout exists to bound. They end when the
        // killed group closes the pipes.
        kill_group(&mut child);
        return Err(match wait_error {
            Some(source) => ProcessError::Spawn {
                program: program.to_owned(),
                source,
            },
            None => ProcessError::TimedOut {
                program: program.to_owned(),
                args: args.join(" "),
                timeout,
            },
        });
    };

    // The child exited on its own; the readers see EOF once every process it
    // spawned has closed the pipes.
    let stdout = stdout.join().expect("the pipe reader does not panic");
    let stderr = stderr.join().expect("the pipe reader does not panic");
    Ok(ProcessOutput {
        success: status.success(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

/// Put the child at the head of a fresh process group, so a signal can reach
/// everything it forks.
#[cfg(unix)]
fn lead_its_own_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// Off Unix there are no process groups to lead; the timeout still stops the
/// direct child.
#[cfg(not(unix))]
fn lead_its_own_group(_command: &mut Command) {}

/// Stop `child` and everything it spawned, then reap it.
///
/// The child leads its own process group, so signalling the group reaches a
/// forked grandchild that inherited the pipes and not just the direct child.
/// The group signal goes through [`rustix::process::kill_process_group`], a
/// safe wrapper around `kill(2)` — `unsafe` is forbidden crate-wide, and
/// shelling out to the `kill` binary would leave a forked grandchild holding
/// the pipes on an image that lacks it. When the group cannot be signalled the
/// direct child is killed on its own.
fn kill_group(child: &mut Child) {
    if !kill_the_group(child) {
        let _ = child.kill();
    }
    let _ = child.wait();
}

/// Send `SIGKILL` to the process group led by `child`, reporting whether it was
/// delivered.
#[cfg(unix)]
fn kill_the_group(child: &Child) -> bool {
    use rustix::process::{Pid, Signal};
    rustix::process::kill_process_group(Pid::from_child(child), Signal::KILL).is_ok()
}

/// Off Unix there are no process groups to signal; the direct child is killed.
#[cfg(not(unix))]
fn kill_the_group(_child: &Child) -> bool {
    false
}

/// Turn a non-zero exit into [`ProcessError::Failed`], leaving a successful run
/// untouched.
pub(crate) fn require_success(
    program: &str,
    args: &[&str],
    output: ProcessOutput,
) -> Result<ProcessOutput, ProcessError> {
    if output.success {
        Ok(output)
    } else {
        Err(ProcessError::Failed {
            program: program.to_owned(),
            args: args.join(" "),
            stderr: output.stderr.trim().to_owned(),
        })
    }
}

/// Read a pipe to end, ignoring a read error (the output is best-effort text).
fn read_all(mut pipe: impl Read) -> Vec<u8> {
    let mut buffer = Vec::new();
    let _ = pipe.read_to_end(&mut buffer);
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashMap;

    /// The built command's environment, as a map for lookup.
    fn envs(command: &Command) -> HashMap<String, Option<String>> {
        command
            .get_envs()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.map(|value| value.to_string_lossy().into_owned()),
                )
            })
            .collect()
    }

    /// A scratch directory for the process tests.
    fn cwd() -> tempfile::TempDir {
        tempfile::TempDir::new().expect("temp dir")
    }

    #[test]
    fn a_command_carries_the_non_interactive_environment() {
        let command = command("git", &["status"], Path::new("/repo"));
        let envs = envs(&command);

        for (key, value) in NON_INTERACTIVE {
            assert_eq!(
                envs.get(key).and_then(Option::as_deref),
                Some(value),
                "`{key}` is not set to `{value}`"
            );
        }
    }

    #[test]
    fn a_command_carries_the_program_arguments_and_directory() {
        let command = command("mise", &["tasks", "graph", "--json"], Path::new("/repo"));

        assert_eq!(command.get_program().to_string_lossy(), "mise");
        assert_eq!(command.get_current_dir(), Some(Path::new("/repo")));
        let args: Vec<&str> = command
            .get_args()
            .map(|arg| arg.to_str().expect("a UTF-8 argument"))
            .collect();
        assert_eq!(args, ["tasks", "graph", "--json"]);
    }

    #[test]
    fn a_successful_command_reports_its_output() {
        let dir = cwd();
        let output = run(
            "sh",
            &["-c", "printf hello; printf oops >&2"],
            dir.path(),
            secs(5),
        )
        .expect("the command runs");

        assert!(output.success);
        assert_eq!(output.stdout, "hello");
        assert_eq!(output.stderr, "oops");
    }

    #[test]
    fn a_non_zero_exit_is_not_an_error_but_require_success_rejects_it() {
        let dir = cwd();
        let output = run("sh", &["-c", "exit 3"], dir.path(), secs(5)).expect("the command runs");
        assert!(!output.success);

        let err = require_success("sh", &["-c", "exit 3"], output).expect_err("non-zero fails");
        assert!(matches!(err, ProcessError::Failed { ref program, .. } if program == "sh"));
    }

    #[test]
    fn a_forking_child_is_bounded_by_the_timeout_and_its_group_is_killed() {
        // `sh -c` forks a grandchild, which inherits the pipes: killing only the
        // direct child leaves the grandchild holding them and the call blocks
        // for the full sleep. The grandchild writes a marker a moment later, so
        // this proves both that the call returns promptly and that the group —
        // not just the direct child — was stopped.
        let dir = cwd();
        let marker = dir.path().join("marker").to_string_lossy().into_owned();
        let script = r#"(sleep 0.3; echo done > "$1") & sleep 30"#;

        let started = Instant::now();
        let err = run(
            "sh",
            &["-c", script, "sh", &marker],
            dir.path(),
            Duration::from_millis(50),
        )
        .expect_err("the forked sleep outlives the timeout");

        assert!(
            matches!(err, ProcessError::TimedOut { ref timeout, .. } if *timeout == Duration::from_millis(50)),
            "{err:?}"
        );
        assert_eq!(err.timeout(), Some(Duration::from_millis(50)));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "the timeout did not bound a forking child: {:?}",
            started.elapsed()
        );

        // Give the grandchild time to have written the marker had it survived.
        thread::sleep(Duration::from_millis(600));
        assert!(
            !Path::new(&marker).exists(),
            "the forked grandchild survived the timeout"
        );
    }

    /// A timeout from whole seconds, for readability.
    fn secs(seconds: u64) -> Duration {
        Duration::from_secs(seconds)
    }

    #[test]
    fn a_missing_program_is_a_spawn_error() {
        let dir = cwd();
        let err = run("momo-no-such-program", &[], dir.path(), secs(5))
            .expect_err("the program does not exist");
        assert!(matches!(err, ProcessError::Spawn { .. }), "{err:?}");
        assert_eq!(err.timeout(), None);
    }

    #[test]
    fn a_spawn_error_names_the_program() {
        let dir = cwd();
        let err = run("momo-no-such-program", &[], dir.path(), secs(5))
            .expect_err("the program does not exist");
        assert!(err.to_string().contains("momo-no-such-program"), "{err}");
    }
}
