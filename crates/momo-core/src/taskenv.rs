//! The `TaskEnvironment` port: installing momo's task wrappers into an external
//! task runner.
//!
//! Adapters live in `momo-taskenv` (mise in v0); the domain never touches the
//! filesystem. A caller injects whichever adapter fits — a real one in
//! production, a fake in tests — so the domain logic stays free of I/O.
//!
//! Installing is split into a read-only [`TaskEnvironment::plan`] and a
//! [`TaskEnvironment::write`], which lets a caller show a `--dry-run` without
//! mutating anything and keeps idempotency (unchanged files are never written)
//! testable at the seam.

use std::path::{Path, PathBuf};

use semver::Version;

/// How [`TaskEnvironment::plan`] reconciles momo's wrappers with the disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallMode {
    /// Create missing wrappers and leave any existing wrapper untouched.
    Install,
    /// Create missing wrappers and refresh existing ones to the running version.
    Update,
}

/// One generated task wrapper: where it goes and what it contains.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskWrapper {
    /// The absolute path the wrapper is written to.
    pub path: PathBuf,
    /// The wrapper's contents.
    pub contents: String,
}

/// The change installing would make on disk.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct InstallPlan {
    /// Wrappers to create or refresh.
    pub writes: Vec<TaskWrapper>,
    /// Wrapper paths that already match the desired contents.
    pub unchanged: Vec<PathBuf>,
    /// Existing wrappers left as-is because they differ from the desired contents.
    pub stale: Vec<PathBuf>,
}

impl InstallPlan {
    /// Whether applying the plan would leave the disk exactly as it is.
    pub fn is_noop(&self) -> bool {
        self.writes.is_empty() && self.stale.is_empty()
    }
}

/// The task-environment operations momo needs.
///
/// The environment is rooted at [`TaskEnvironment::root`]; momo's wrappers live
/// under [`TaskEnvironment::tasks_dir`]. Every method is side-effect-free except
/// [`TaskEnvironment::write`] and [`TaskEnvironment::uninstall`].
pub trait TaskEnvironment {
    /// The environment's canonical name, e.g. `mise`.
    fn name(&self) -> &'static str;

    /// The directory the environment is rooted at.
    fn root(&self) -> &Path;

    /// The directory holding momo's task wrappers.
    fn tasks_dir(&self) -> &Path;

    /// The change installing `verbs` at `version` would make.
    ///
    /// `mode` decides what happens to a wrapper that exists but does not match:
    /// [`InstallMode::Install`] leaves it and reports it as stale, while
    /// [`InstallMode::Update`] refreshes it.
    fn plan(
        &self,
        version: &Version,
        verbs: &[&str],
        mode: InstallMode,
    ) -> Result<InstallPlan, TaskEnvError>;

    /// Write `wrappers`, creating the task directory as needed.
    fn write(&self, wrappers: &[TaskWrapper]) -> Result<(), TaskEnvError>;

    /// Remove momo's wrappers. Removing when nothing is installed is not an error.
    fn uninstall(&self) -> Result<(), TaskEnvError>;
}

/// The domain errors a caller must be able to match on.
#[derive(Debug, thiserror::Error)]
pub enum TaskEnvError {
    /// The requested environment name is not one momo supports.
    #[error("unknown task environment `{name}`; expected: mise")]
    Unknown {
        /// The unrecognised name.
        name: String,
    },

    /// A wrapper could not be read.
    #[error("failed to read `{path}`")]
    Read {
        /// The path that could not be read.
        path: PathBuf,
        /// Why the read failed.
        #[source]
        source: std::io::Error,
    },

    /// A wrapper or directory could not be written.
    #[error("failed to write `{path}`")]
    Write {
        /// The path that could not be written.
        path: PathBuf,
        /// Why the write failed.
        #[source]
        source: std::io::Error,
    },

    /// momo's wrapper directory could not be removed.
    #[error("failed to remove `{path}`")]
    Remove {
        /// The directory that could not be removed.
        path: PathBuf,
        /// Why the removal failed.
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_plan_is_a_noop() {
        assert!(InstallPlan::default().is_noop());
    }

    #[test]
    fn a_plan_with_writes_or_stale_is_not_a_noop() {
        let wrapper = TaskWrapper {
            path: PathBuf::from("/repo/mise-tasks/momo/status"),
            contents: String::new(),
        };
        let with_write = InstallPlan {
            writes: vec![wrapper],
            ..InstallPlan::default()
        };
        assert!(!with_write.is_noop());

        let with_stale = InstallPlan {
            stale: vec![PathBuf::from("/repo/mise-tasks/momo/status")],
            ..InstallPlan::default()
        };
        assert!(!with_stale.is_noop());
    }

    #[test]
    fn a_plan_of_only_unchanged_files_is_a_noop() {
        let plan = InstallPlan {
            unchanged: vec![PathBuf::from("/repo/mise-tasks/momo/status")],
            ..InstallPlan::default()
        };
        assert!(plan.is_noop());
    }
}
