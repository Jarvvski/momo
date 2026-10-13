//! Building child processes with a non-interactive environment.
//!
//! Every external command momo spawns runs unattended — in CI, in the release
//! job — so it must never stop to prompt. A git credential prompt, an editor, a
//! pager or a mise trust prompt would hang the process with no diagnostic.
//! [`command`] is the one place a [`Command`] is built, so the environment is
//! applied uniformly and can be asserted without spawning a process.

use std::path::Path;
use std::process::Command;

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

/// A [`Command`] for `program` run with `args` in `cwd`.
///
/// The command carries the non-interactive environment, so callers spawn it
/// without thinking about prompts.
pub(crate) fn command(program: &str, args: &[&str], cwd: &Path) -> Command {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd).envs(NON_INTERACTIVE);
    command
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
}
