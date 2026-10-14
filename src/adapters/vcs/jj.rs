//! The jj adapter: drives a repository through the `jj` CLI.
//!
//! Tags are **lightweight** (`jj tag set`) — jj cannot create annotated tags.

use std::path::{Path, PathBuf};

use crate::domain::vcs::{Vcs, VcsError, VcsKind};
use semver::Version;

use super::process::{CommandRunner, require_success};
use super::tags::{checked_tag_name, latest_release, latest_version};
use crate::adapters::spawn::ProcessOutput;

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

    fn latest_release_tag(&self, prefix: &str) -> Result<Option<Version>, VcsError> {
        let args = ["tag", "list"];
        let output = require_success("jj", &args, self.jj(&args)?)?;
        Ok(latest_release(
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

    fn create_tag(
        &self,
        rev: Option<&str>,
        prefix: &str,
        version: &Version,
        _message: &str,
    ) -> Result<(), VcsError> {
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

        // The documented flow is release → commit → tag, so the release is the
        // parent of the empty working-copy commit: anchor to `@-` by default.
        // `@` is only right for an uncommitted release, which the caller names
        // explicitly with `--rev @`; inferring it from the working copy's
        // dirtiness mis-anchored whenever an unrelated file made `@` dirty.
        let revision = rev.unwrap_or("@-");
        let set = ["tag", "set", tag.as_str(), "-r", revision];
        require_success("jj", &set, self.jj(&set)?)?;
        Ok(())
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
    fn create_tag_defaults_to_the_parent_even_when_the_working_copy_is_dirty() {
        // The documented flow is release → commit → tag, so the release sits at
        // `@-`. An unrelated stray file makes `@` dirty — jj auto-tracks it —
        // but the anchor must not follow: the tag names `@-`, not the empty
        // working-copy commit. No `diff` probe is scripted, so the fake would
        // panic if the adapter still consulted dirtiness.
        let runner = FakeRunner::new().ok("jj", &["tag", "list"], "").ok(
            "jj",
            &["tag", "set", "v0.1.0", "-r", "@-"],
            "",
        );
        adapter(runner)
            .create_tag(None, "v", &Version::new(0, 1, 0), "Release 0.1.0")
            .unwrap();
    }

    #[test]
    fn latest_release_tag_skips_candidates() {
        let runner = FakeRunner::new().ok(
            "jj",
            &["tag", "list"],
            "v0.1.0: abc123 first\nv0.2.0-rc.1: def456 second\n",
        );
        assert_eq!(
            adapter(runner).latest_release_tag("v").unwrap(),
            Some(Version::new(0, 1, 0))
        );
    }

    #[test]
    fn create_tag_honours_an_explicit_revision() {
        // An uncommitted release names `@` explicitly; the tag then points at
        // the working-copy commit.
        let runner = FakeRunner::new().ok("jj", &["tag", "list"], "").ok(
            "jj",
            &["tag", "set", "v0.1.0", "-r", "@"],
            "",
        );
        adapter(runner)
            .create_tag(Some("@"), "v", &Version::new(0, 1, 0), "Release 0.1.0")
            .unwrap();
    }

    #[test]
    fn create_tag_refuses_to_overwrite_an_existing_tag() {
        let runner = FakeRunner::new().ok("jj", &["tag", "list"], "v0.1.0: abc123 first\n");
        let err = adapter(runner)
            .create_tag(None, "v", &Version::new(0, 1, 0), "Release 0.1.0")
            .unwrap_err();
        assert!(matches!(err, VcsError::TagAlreadyExists { ref tag } if tag == "v0.1.0"));
    }

    #[test]
    fn create_tag_rejects_an_invalid_name_without_spawning() {
        // An empty script panics on any command, proving validation runs first.
        let runner = FakeRunner::new();
        let err = adapter(runner)
            .create_tag(None, "v 1", &Version::new(2, 3, 0), "Release 2.3.0")
            .unwrap_err();
        assert!(matches!(err, VcsError::InvalidTagName { ref name } if name == "v 12.3.0"));
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
