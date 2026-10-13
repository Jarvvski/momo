//! The git adapter: drives a repository through the `git` CLI.
//!
//! Tags are **annotated** (`git tag --annotate`), the form git intends for a
//! release.

use std::path::{Path, PathBuf};

use momo_core::vcs::{Vcs, VcsError, VcsKind};
use semver::Version;

use crate::process::{CommandRunner, ProcessOutput, require_success};
use crate::tags::{latest_version, tag_name};

/// A git repository, driven through the `git` CLI.
pub struct GitVcs<R> {
    runner: R,
    root: PathBuf,
}

impl<R: CommandRunner> GitVcs<R> {
    /// Root a git adapter at `root` and drive it with `runner`.
    pub fn new(runner: R, root: impl Into<PathBuf>) -> Self {
        Self {
            runner,
            root: root.into(),
        }
    }

    fn git(&self, args: &[&str]) -> Result<ProcessOutput, VcsError> {
        self.runner.run("git", args, &self.root)
    }
}

impl<R: CommandRunner> Vcs for GitVcs<R> {
    fn kind(&self) -> VcsKind {
        VcsKind::Git
    }

    fn root(&self) -> &Path {
        &self.root
    }

    fn latest_tag(&self, prefix: &str) -> Result<Option<Version>, VcsError> {
        let args = ["tag", "--list"];
        let output = require_success("git", &args, self.git(&args)?)?;
        Ok(latest_version(output.stdout.lines(), prefix))
    }

    fn remote(&self) -> Result<Option<String>, VcsError> {
        // `get-url` exits non-zero when `origin` is not configured; that is not
        // an error here, it just means there is no remote to link to.
        let args = ["remote", "get-url", "origin"];
        let output = self.git(&args)?;
        Ok(match output.stdout.trim() {
            "" => None,
            url => Some(url.to_owned()),
        })
    }

    fn create_tag(&self, prefix: &str, version: &Version, message: &str) -> Result<(), VcsError> {
        let tag = tag_name(prefix, version);
        let reference = format!("refs/tags/{tag}");
        let verify = ["rev-parse", "--quiet", "--verify", reference.as_str()];
        if self.git(&verify)?.success {
            return Err(VcsError::TagAlreadyExists { tag });
        }

        let create = ["tag", "--annotate", "--message", message, tag.as_str()];
        require_success("git", &create, self.git(&create)?)?;
        Ok(())
    }

    fn is_dirty(&self) -> Result<bool, VcsError> {
        let args = ["status", "--porcelain"];
        let output = require_success("git", &args, self.git(&args)?)?;
        Ok(!output.stdout.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeRunner;

    fn adapter(runner: FakeRunner) -> GitVcs<FakeRunner> {
        GitVcs::new(runner, "/repo")
    }

    #[test]
    fn latest_tag_picks_the_highest_matching_version() {
        let runner = FakeRunner::new().ok(
            "git",
            &["tag", "--list"],
            "v0.1.0\nv0.10.0\nv0.2.0\nnightly\nother-v9.9.9\n",
        );
        assert_eq!(
            adapter(runner).latest_tag("v").unwrap(),
            Some(Version::new(0, 10, 0))
        );
    }

    #[test]
    fn latest_tag_reports_none_when_nothing_matches() {
        let runner = FakeRunner::new().ok("git", &["tag", "--list"], "other-v1.0.0\n");
        assert_eq!(adapter(runner).latest_tag("v").unwrap(), None);
    }

    #[test]
    fn create_tag_writes_an_annotated_tag() {
        // Any command other than these two makes the fake panic, so this pins
        // both the existence check and the annotated form.
        let runner = FakeRunner::new()
            .fail(
                "git",
                &["rev-parse", "--quiet", "--verify", "refs/tags/v0.1.0"],
                "",
            )
            .ok(
                "git",
                &["tag", "--annotate", "--message", "Release 0.1.0", "v0.1.0"],
                "",
            );
        adapter(runner)
            .create_tag("v", &Version::new(0, 1, 0), "Release 0.1.0")
            .unwrap();
    }

    #[test]
    fn create_tag_refuses_to_overwrite_an_existing_tag() {
        let runner = FakeRunner::new().ok(
            "git",
            &["rev-parse", "--quiet", "--verify", "refs/tags/v0.1.0"],
            "f665a604ff51b0e3156f31bfa862e61acf77da52\n",
        );
        let err = adapter(runner)
            .create_tag("v", &Version::new(0, 1, 0), "Release 0.1.0")
            .unwrap_err();
        assert!(matches!(err, VcsError::TagAlreadyExists { ref tag } if tag == "v0.1.0"));
    }

    #[test]
    fn is_dirty_tracks_the_status_output() {
        let dirty = FakeRunner::new().ok("git", &["status", "--porcelain"], " M a.txt\n");
        assert!(adapter(dirty).is_dirty().unwrap());

        let clean = FakeRunner::new().ok("git", &["status", "--porcelain"], "");
        assert!(!adapter(clean).is_dirty().unwrap());
    }

    #[test]
    fn remote_reads_origins_url() {
        let runner = FakeRunner::new().ok(
            "git",
            &["remote", "get-url", "origin"],
            "git@github.com:Jarvvski/momo.git\n",
        );
        assert_eq!(
            adapter(runner).remote().unwrap().as_deref(),
            Some("git@github.com:Jarvvski/momo.git")
        );
    }

    #[test]
    fn remote_reports_none_when_there_is_no_origin() {
        // `git remote get-url origin` exits non-zero with an empty stdout.
        let runner = FakeRunner::new().fail("git", &["remote", "get-url", "origin"], "");
        assert_eq!(adapter(runner).remote().unwrap(), None);
    }
}
