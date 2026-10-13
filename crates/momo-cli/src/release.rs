//! The Release lifecycle: minting a Version, promoting the Changelog, consuming
//! Fragments, and creating the VCS tag.
//!
//! Everything here is orchestration. The pure decisions — the Bump, the
//! Changelog text, the reference links — live in `momo-core`; the file work goes
//! through [`crate::recording`] and [`FileVersionSource`], and the repository is
//! touched only through the injected [`Vcs`].

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use momo_core::changelog::{Changelog, Links};
use momo_core::config::Config;
use momo_core::remote::Remote;
use momo_core::vcs::Vcs;
use momo_core::version::{
    DEFAULT_RC_FORMAT, VersionSource, candidate_version, next_candidate, release_version,
};
use semver::Version;
use time::Date;

use crate::recording;
use crate::version::{FileVersionSource, VERSION_FILE, source};

/// What a `release` or `candidate` run did, or would do under `--dry-run`.
#[derive(Debug)]
pub struct Outcome {
    /// The Version minted.
    pub version: Version,
    /// The Changelog the run wrote.
    pub changelog: PathBuf,
    /// The version file a Release wrote; `None` for a Candidate.
    pub version_file: Option<PathBuf>,
    /// The Fragments a Release consumed.
    pub consumed: Vec<PathBuf>,
}

/// Which end of the Release lifecycle a run is minting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// A Release: consume the Unreleased Changes.
    Release,
    /// A Release Candidate: share them with the eventual Release.
    Candidate,
}

/// Mint a Release from the Unreleased Changes.
///
/// Promotes the Changelog's Unreleased section into `## [x.y.z] - date`,
/// consumes the Fragments, and writes `.momo/version`. It never commits and
/// never tags.
pub fn release(
    root: &Path,
    config: &Config,
    vcs: Option<&dyn Vcs>,
    date: Date,
    dry_run: bool,
) -> Result<Outcome> {
    run(root, config, vcs, date, Mode::Release, dry_run)
}

/// Cut a Release Candidate from the Unreleased Changes.
///
/// Bumps exactly as a Release would, then appends `-rc.N`. The same files are
/// written, but the Fragments are left in place: a Candidate shares them with
/// the eventual Release.
pub fn candidate(
    root: &Path,
    config: &Config,
    vcs: Option<&dyn Vcs>,
    date: Date,
    dry_run: bool,
) -> Result<Outcome> {
    run(root, config, vcs, date, Mode::Candidate, dry_run)
}

/// Create the VCS tag for the current Version.
///
/// The tag names the Version `.momo/version` holds — the one the last Release
/// minted — so `tag` brings the repository and momo's own record into sync.
/// Returns the tag name; the repository is untouched under `dry_run`.
pub fn tag(root: &Path, config: &Config, vcs: &dyn Vcs, dry_run: bool) -> Result<String> {
    let file = FileVersionSource::new(root);
    if !file.exists() {
        bail!("nothing to tag: `{VERSION_FILE}` does not exist; run `momo release` first");
    }

    let version = file.current()?;
    let message = format!("Release {version}");
    if !dry_run {
        vcs.create_tag(&config.tag_prefix, &version, &message)?;
    }
    Ok(format!("{}{version}", config.tag_prefix))
}

/// The shared body of [`release`] and [`candidate`].
fn run(
    root: &Path,
    config: &Config,
    vcs: Option<&dyn Vcs>,
    date: Date,
    mode: Mode,
    dry_run: bool,
) -> Result<Outcome> {
    let changes = recording::unreleased(root, config)?;
    let current = source(root, config, vcs).current()?;
    let Some(target) = release_version(&changes, &current) else {
        bail!("nothing unreleased to release");
    };

    let mut changelog = read_changelog(root, config)?;
    let remote = remote(vcs)?;
    let links = Links {
        prefix: &config.tag_prefix,
        remote: remote.as_ref(),
    };
    let version = match mode {
        Mode::Release => {
            changelog.release(&target, date, &changes, links);
            target
        }
        Mode::Candidate => {
            let format = rc_format(config);
            let recorded = changelog.released_versions();
            let n = next_candidate(
                &target,
                format,
                recorded.iter().chain(std::iter::once(&current)),
            );
            let version = candidate_version(&target, n, format)?;
            changelog.candidate(&version, date, &changes, links);
            version
        }
    };

    let changelog_path = root.join(&config.changelog);
    if !dry_run {
        fs::write(&changelog_path, changelog.render())
            .with_context(|| format!("writing {}", changelog_path.display()))?;
    }

    let consumed = match mode {
        Mode::Release => recording::consume(root, config, dry_run)?,
        Mode::Candidate => Vec::new(),
    };

    // Both verbs advance the version source. A Candidate has to be taggable —
    // that is the point of cutting one — and the eventual Release reads the
    // Candidate's version back and drops the `-rc.N` suffix.
    if !dry_run {
        FileVersionSource::new(root).write(&version)?;
    }
    let version_file = Some(root.join(VERSION_FILE));

    Ok(Outcome {
        version,
        changelog: changelog_path,
        version_file,
        consumed,
    })
}

/// The Changelog on disk, or a new one when the file is missing.
fn read_changelog(root: &Path, config: &Config) -> Result<Changelog> {
    let path = root.join(&config.changelog);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(Changelog::parse(&text)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(Changelog::empty()),
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}

/// The Remote to link to, parsed from the VCS remote when one is configured.
///
/// An unrecognised remote URL means there is nothing to link to.
fn remote(vcs: Option<&dyn Vcs>) -> Result<Option<Remote>> {
    let Some(vcs) = vcs else {
        return Ok(None);
    };
    let Some(url) = vcs.remote()? else {
        return Ok(None);
    };
    Ok(url.parse::<Remote>().ok())
}

/// The Release Candidate suffix template from the configuration.
fn rc_format(config: &Config) -> &str {
    config.rc_format.as_deref().unwrap_or(DEFAULT_RC_FORMAT)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    use momo_core::config::Recording;
    use momo_core::vcs::{VcsError, VcsKind};
    use tempfile::TempDir;

    /// A [`Vcs`] that reports fixed state and never touches a repository.
    struct FakeVcs {
        latest: Option<Version>,
        remote: Option<String>,
    }

    impl FakeVcs {
        fn new() -> Self {
            Self {
                latest: None,
                remote: None,
            }
        }

        fn latest(mut self, version: Version) -> Self {
            self.latest = Some(version);
            self
        }

        fn remote(mut self, url: &str) -> Self {
            self.remote = Some(url.to_owned());
            self
        }
    }

    impl Vcs for FakeVcs {
        fn kind(&self) -> VcsKind {
            VcsKind::Jj
        }

        fn root(&self) -> &Path {
            Path::new("/repo")
        }

        fn latest_tag(&self, _prefix: &str) -> Result<Option<Version>, VcsError> {
            Ok(self.latest.clone())
        }

        fn remote(&self) -> Result<Option<String>, VcsError> {
            Ok(self.remote.clone())
        }

        fn create_tag(
            &self,
            _prefix: &str,
            _version: &Version,
            _message: &str,
        ) -> Result<(), VcsError> {
            Ok(())
        }

        fn is_dirty(&self) -> Result<bool, VcsError> {
            Ok(false)
        }
    }

    /// The date every test stamps its Release with.
    fn date() -> Date {
        Date::from_calendar_date(2026, time::Month::October, 13).expect("a valid date")
    }

    /// The fragment recording model.
    fn fragments() -> Config {
        Config {
            recording: Recording::Fragments,
            ..Config::default()
        }
    }

    /// Seed one Fragment under the project's changes directory.
    fn fragment(root: &Path, name: &str, text: &str) {
        let dir = root.join(".momo/changes");
        fs::create_dir_all(&dir).expect("create the changes dir");
        fs::write(dir.join(name), format!("{text}\n")).expect("write a fragment");
    }

    /// Read a file the test expects the run to have written.
    fn read(root: &Path, path: &str) -> String {
        fs::read_to_string(root.join(path)).expect("read a written file")
    }

    #[test]
    fn a_release_promotes_the_changelog_and_consumes_the_fragments() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-add-a-thing.md", "Add a thing");
        fragment(root, "fix-fix-a-thing.md", "Fix a thing");

        let outcome = release(root, &fragments(), None, date(), false).expect("release");

        assert_eq!(outcome.version, Version::new(0, 1, 0));
        assert_eq!(outcome.consumed.len(), 2);
        assert!(!root.join(".momo/changes/add-add-a-thing.md").exists());
        assert!(!root.join(".momo/changes/fix-fix-a-thing.md").exists());

        let changelog = read(root, "CHANGELOG.md");
        assert!(changelog.contains("## [0.1.0] - 2026-10-13"), "{changelog}");
        assert!(changelog.contains("- Add a thing"), "{changelog}");
        assert!(changelog.contains("- Fix a thing"), "{changelog}");
    }

    #[test]
    fn a_release_writes_the_version_file() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");

        release(root, &fragments(), None, date(), false).expect("release");

        assert_eq!(read(root, ".momo/version"), "0.1.0\n");
    }

    #[test]
    fn a_release_with_nothing_unreleased_errors() {
        let dir = TempDir::new().expect("temp dir");

        let err = release(dir.path(), &Config::default(), None, date(), false)
            .expect_err("an empty release fails");
        assert!(err.to_string().contains("nothing unreleased"), "{err}");
    }

    #[test]
    fn a_release_takes_its_version_from_the_tag_when_a_vcs_is_present() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-more.md", "More");
        let vcs = FakeVcs::new().latest(Version::new(0, 1, 0));

        let outcome = release(root, &fragments(), Some(&vcs), date(), false).expect("release");

        assert_eq!(outcome.version, Version::new(0, 2, 0));
    }

    #[test]
    fn candidates_increment_and_restart_when_the_target_changes() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        let config = fragments();
        fragment(root, "add-a-thing.md", "Add a thing");

        let first = candidate(root, &config, None, date(), false).expect("first candidate");
        assert_eq!(first.version.to_string(), "0.1.0-rc.1");

        let second = candidate(root, &config, None, date(), false).expect("second candidate");
        assert_eq!(second.version.to_string(), "0.1.0-rc.2");

        // A Release drops the suffix and consumes the shared Fragments.
        let released = release(root, &config, None, date(), false).expect("release");
        assert_eq!(released.version.to_string(), "0.1.0");

        // A new Change targets 0.2.0, so the counter restarts at 1.
        fragment(root, "add-another.md", "Another thing");
        let third = candidate(root, &config, None, date(), false).expect("third candidate");
        assert_eq!(third.version.to_string(), "0.2.0-rc.1");
    }

    #[test]
    fn a_candidate_shares_the_fragments_with_the_eventual_release() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");

        let outcome = candidate(root, &fragments(), None, date(), false).expect("candidate");

        assert!(outcome.consumed.is_empty());
        assert!(root.join(".momo/changes/add-a-thing.md").exists());
        // The Candidate still advances the version source, so `tag` can name it.
        let version_file = root.join(VERSION_FILE);
        assert_eq!(
            outcome.version_file.as_deref(),
            Some(version_file.as_path())
        );
        assert_eq!(
            fs::read_to_string(&version_file).expect("read the version file"),
            "0.1.0-rc.1\n"
        );
    }

    #[test]
    fn a_release_writes_reference_links_when_a_remote_is_known() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        let vcs = FakeVcs::new().remote("git@github.com:Jarvvski/momo.git");

        release(root, &fragments(), Some(&vcs), date(), false).expect("release");

        let changelog = read(root, "CHANGELOG.md");
        assert!(
            changelog.contains("[0.1.0]: https://github.com/Jarvvski/momo/releases/tag/v0.1.0"),
            "{changelog}"
        );
    }

    #[test]
    fn a_dry_run_release_writes_nothing() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");

        let outcome = release(root, &fragments(), None, date(), true).expect("dry-run release");

        assert_eq!(outcome.version, Version::new(0, 1, 0));
        assert!(!root.join("CHANGELOG.md").exists());
        assert!(!root.join(".momo/version").exists());
        assert!(root.join(".momo/changes/add-a-thing.md").exists());
    }

    #[test]
    fn a_tag_names_the_version_momo_last_released() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        FileVersionSource::new(root)
            .write(&Version::new(1, 2, 3))
            .expect("write the version file");

        let name = tag(root, &Config::default(), &FakeVcs::new(), false).expect("tag");

        assert_eq!(name, "v1.2.3");
    }

    #[test]
    fn a_tag_names_the_version_a_candidate_minted() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");

        let outcome = candidate(root, &fragments(), None, date(), false).expect("candidate");
        assert_eq!(outcome.version.to_string(), "0.1.0-rc.1");

        let name = tag(root, &fragments(), &FakeVcs::new(), false).expect("tag");

        assert_eq!(name, "v0.1.0-rc.1");
    }

    #[test]
    fn a_tag_without_a_release_errors() {
        let dir = TempDir::new().expect("temp dir");

        let err = tag(dir.path(), &Config::default(), &FakeVcs::new(), false)
            .expect_err("tagging with nothing released fails");
        assert!(
            err.to_string().contains("run `momo release` first"),
            "{err}"
        );
    }
}
