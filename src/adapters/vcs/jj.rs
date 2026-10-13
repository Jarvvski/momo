//! The jj adapter: drives a repository through the `jj` CLI.
//!
//! Tags are **lightweight** (`jj tag set`) — jj cannot create annotated tags.

use std::path::{Path, PathBuf};

use crate::domain::vcs::{Vcs, VcsError, VcsKind};
use semver::Version;

use super::process::{CommandRunner, ProcessOutput, require_success};
use super::tags::{checked_tag_name, latest_version};

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

    fn remote(&self) -> Result<Option<String>, VcsError> {
        // `jj git remote list` prints one `<name> <url>` line per remote.
        let args = ["git", "remote", "list"];
        let output = require_success("jj", &args, self.jj(&args)?)?;
        let mut first = None;
        for line in output.stdout.lines() {
            let Some((name, url)) = line.split_once(' ') else {
                continue;
            };
            let url = url.trim().to_owned();
            if name == "origin" {
                return Ok(Some(url));
            }
            first.get_or_insert(url);
        }
        Ok(first)
    }

    fn create_tag(&self, prefix: &str, version: &Version, _message: &str) -> Result<(), VcsError> {
        let tag = checked_tag_name(prefix, version)?;
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

        // jj defaults `-r` to `@`, the working-copy commit. That is right only
        // when the working copy holds the release; after `jj commit`, or above
        // `main` after a merge, `@` is an empty commit, so tagging it would
        // publish a dangling commit. Anchor to the parent when the working copy
        // has no changes of its own.
        let revision = if self.is_dirty()? { "@" } else { "@-" };
        let set = ["tag", "set", tag.as_str(), "-r", revision];
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
    use crate::adapters::vcs::test_support::FakeRunner;

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
    fn create_tag_anchors_to_a_dirty_working_copy() {
        // When `@` holds the release, the tag must name `@` — a lightweight
        // `jj tag set` (no message), which is all jj supports.
        let runner = FakeRunner::new()
            .ok("jj", &["tag", "list"], "")
            .ok("jj", &["diff", "--summary"], "M CHANGELOG.md\n")
            .ok("jj", &["tag", "set", "v0.1.0", "-r", "@"], "");
        adapter(runner)
            .create_tag("v", &Version::new(0, 1, 0), "Release 0.1.0")
            .unwrap();
    }

    #[test]
    fn create_tag_anchors_to_the_parent_of_an_empty_working_copy() {
        // After `jj commit` — or above `main` after a merge — `@` is an empty
        // commit; the tag must name its parent instead, or it would publish a
        // dangling commit.
        let runner = FakeRunner::new()
            .ok("jj", &["tag", "list"], "")
            .ok("jj", &["diff", "--summary"], "")
            .ok("jj", &["tag", "set", "v0.1.0", "-r", "@-"], "");
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
    fn create_tag_rejects_an_invalid_name_without_spawning() {
        // An empty script panics on any command, proving validation runs first.
        let runner = FakeRunner::new();
        let err = adapter(runner)
            .create_tag("v 1", &Version::new(2, 3, 0), "Release 2.3.0")
            .unwrap_err();
        assert!(matches!(err, VcsError::InvalidTagName { ref name } if name == "v 12.3.0"));
    }

    #[test]
    fn is_dirty_tracks_the_diff_summary() {
        let dirty = FakeRunner::new().ok("jj", &["diff", "--summary"], "M a.txt\n");
        assert!(adapter(dirty).is_dirty().unwrap());

        let clean = FakeRunner::new().ok("jj", &["diff", "--summary"], "");
        assert!(!adapter(clean).is_dirty().unwrap());
    }

    #[test]
    fn remote_prefers_origin_over_other_remotes() {
        let runner = FakeRunner::new().ok(
            "jj",
            &["git", "remote", "list"],
            "upstream git@github.com:other/momo.git\norigin git@github.com:Jarvvski/momo.git\n",
        );
        assert_eq!(
            adapter(runner).remote().unwrap().as_deref(),
            Some("git@github.com:Jarvvski/momo.git")
        );
    }

    #[test]
    fn remote_falls_back_to_the_first_remote_and_reports_none_when_empty() {
        let runner = FakeRunner::new().ok(
            "jj",
            &["git", "remote", "list"],
            "upstream git@github.com:other/momo.git\n",
        );
        assert_eq!(
            adapter(runner).remote().unwrap().as_deref(),
            Some("git@github.com:other/momo.git")
        );

        let none = FakeRunner::new().ok("jj", &["git", "remote", "list"], "");
        assert_eq!(adapter(none).remote().unwrap(), None);
    }
}
