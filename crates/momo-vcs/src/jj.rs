//! The jj adapter: drives a repository through the `jj` CLI.
//!
//! Tags are **lightweight** (`jj tag set`) — jj cannot create annotated tags.

use std::path::{Path, PathBuf};

use momo_core::vcs::{Vcs, VcsError, VcsKind};
use semver::Version;

use crate::process::{CommandRunner, ProcessOutput, require_success};
use crate::tags::{latest_version, tag_name};

/// A jj repository, driven through the `jj` CLI.
pub struct JjVcs<R> {
    runner: R,
    root: PathBuf,
}

impl<R: CommandRunner> JjVcs<R> {
    /// Root a jj adapter at `root` and drive it with `runner`.
    pub fn new(runner: R, root: impl Into<PathBuf>) -> Self {
        Self {
            runner,
            root: root.into(),
        }
    }

    fn jj(&self, args: &[&str]) -> Result<ProcessOutput, VcsError> {
        self.runner.run("jj", args, &self.root)
    }
}

impl<R: CommandRunner> Vcs for JjVcs<R> {
    fn kind(&self) -> VcsKind {
        VcsKind::Jj
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn latest_tag(&self, prefix: &str) -> Result<Option<Version>, VcsError> {
        let args = ["tag", "list"];
        let output = require_success("jj", &args, self.jj(&args)?)?;
        Ok(latest_version(
            output.stdout.lines().map(tag_name_of),
            prefix,
        ))
    }

    fn create_tag(&self, prefix: &str, version: &Version, _message: &str) -> Result<(), VcsError> {
        let tag = tag_name(prefix, version);
        let list = ["tag", "list"];
        let listed = require_success("jj", &list, self.jj(&list)?)?;
        if listed
            .stdout
            .lines()
            .map(tag_name_of)
            .any(|name| name == tag)
        {
            return Err(VcsError::TagAlreadyExists { tag });
        }

        let set = ["tag", "set", tag.as_str()];
        require_success("jj", &set, self.jj(&set)?)?;
        Ok(())
    }

    fn is_dirty(&self) -> Result<bool, VcsError> {
        let args = ["diff", "--summary"];
        let output = require_success("jj", &args, self.jj(&args)?)?;
        Ok(!output.stdout.trim().is_empty())
    }
}

/// `jj tag list` prints `<name>: <target>`; the name is the part before the colon.
fn tag_name_of(line: &str) -> &str {
    line.split_once(": ").map_or(line, |(name, _)| name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeRunner;

    fn adapter(runner: FakeRunner) -> JjVcs<FakeRunner> {
        JjVcs::new(runner, "/repo")
    }

    #[test]
    fn latest_tag_reads_names_from_the_tag_list() {
        let runner = FakeRunner::new().ok(
            "jj",
            &["tag", "list"],
            "v0.1.0: abc123 first\nv0.2.0: def456 second\n",
        );
        assert_eq!(
            adapter(runner).latest_tag("v").unwrap(),
            Some(Version::new(0, 2, 0))
        );
    }

    #[test]
    fn latest_tag_reports_none_when_there_are_no_tags() {
        let runner = FakeRunner::new().ok("jj", &["tag", "list"], "");
        assert_eq!(adapter(runner).latest_tag("v").unwrap(), None);
    }

    #[test]
    fn create_tag_sets_a_lightweight_tag() {
        // `jj tag set <name>` and nothing else: no message, so the tag stays
        // lightweight, which is all jj supports.
        let runner = FakeRunner::new().ok("jj", &["tag", "list"], "").ok(
            "jj",
            &["tag", "set", "v0.1.0"],
            "",
        );
        adapter(runner)
            .create_tag("v", &Version::new(0, 1, 0), "Release 0.1.0")
            .unwrap();
    }

    #[test]
    fn create_tag_refuses_to_overwrite_an_existing_tag() {
        let runner = FakeRunner::new().ok("jj", &["tag", "list"], "v0.1.0: abc123 first\n");
        let err = adapter(runner)
            .create_tag("v", &Version::new(0, 1, 0), "Release 0.1.0")
            .unwrap_err();
        assert!(matches!(err, VcsError::TagAlreadyExists { ref tag } if tag == "v0.1.0"));
    }

    #[test]
    fn is_dirty_tracks_the_diff_summary() {
        let dirty = FakeRunner::new().ok("jj", &["diff", "--summary"], "M a.txt\n");
        assert!(adapter(dirty).is_dirty().unwrap());

        let clean = FakeRunner::new().ok("jj", &["diff", "--summary"], "");
        assert!(!adapter(clean).is_dirty().unwrap());
    }
}
