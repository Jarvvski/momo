//! The Release lifecycle: minting a Version, promoting the Changelog, consuming
//! Fragments, and creating the VCS tag.
//!
//! Everything here is orchestration, per Package. The pure decisions — the Bump,
//! the Changelog text, the reference links — live in `momo-core`; the file work
//! goes through [`crate::recording`] and [`FileVersionSource`], and the
//! repository is touched only through the injected [`Vcs`].

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::domain::changelog::{Changelog, Links};
use crate::domain::config::{Config, Recording};
use crate::domain::manifest::ManifestUpdate;
use crate::domain::package::Package;
use crate::domain::remote::Remote;
use crate::domain::vcs::Vcs;
use crate::domain::version::{
    DEFAULT_RC_FORMAT, VersionSource, candidate_version, next_candidate, release_version,
};
use anyhow::{Context, Result, bail};
use semver::Version;
use time::Date;

use crate::cli::recording;
use crate::cli::version::{FileVersionSource, source};

/// What a `release` or `candidate` run did, or would do under `--dry-run`.
#[derive(Debug)]
pub struct Outcome {
    /// The Version minted.
    pub version: Version,
    /// The Changelog the run wrote.
    pub changelog: PathBuf,
    /// The version file the run wrote. Always present: both `release` and
    /// `candidate` advance it, so the run can be tagged.
    pub version_file: PathBuf,
    /// The Fragments a Release consumed.
    pub consumed: Vec<PathBuf>,
    /// The ecosystem manifest the run synced, when it found one.
    pub manifest: Option<ManifestUpdate>,
}

/// Which end of the Release lifecycle a run is minting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// A Release: consume the Unreleased Changes.
    Release,
    /// A Release Candidate: share them with the eventual Release.
    Candidate,
}

/// Mint a Release for `package` from its Unreleased Changes.
///
/// Promotes the Changelog's Unreleased section into `## [x.y.z] - date`,
/// consumes the Fragments, and writes the Package's version file. It never
/// commits and never tags. Returns `None` when there is nothing Unreleased.
pub fn release(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
    dry_run: bool,
) -> Result<Option<Outcome>> {
    run(
        repo_root,
        config,
        package,
        vcs,
        date,
        Mode::Release,
        dry_run,
    )
}

/// Cut a Release Candidate for `package` from its Unreleased Changes.
///
/// Bumps exactly as a Release would, then appends `-rc.N`. The same files are
/// written, but the Fragments are left in place: a Candidate shares them with
/// the eventual Release. Returns `None` when there is nothing Unreleased.
pub fn candidate(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
    dry_run: bool,
) -> Result<Option<Outcome>> {
    run(
        repo_root,
        config,
        package,
        vcs,
        date,
        Mode::Candidate,
        dry_run,
    )
}

/// Create the VCS tag for `package`'s current Version.
///
/// The tag names the Version the Package's version file holds — the one the last
/// Release minted — so `tag` brings the repository and momo's own record into
/// sync. Returns the tag name, or `None` when the Package has never released.
pub fn tag(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: &dyn Vcs,
    dry_run: bool,
) -> Result<Option<String>> {
    let file = FileVersionSource::new(&package.dir(repo_root));
    if !file.exists() {
        return Ok(None);
    }

    let version = file.current()?;
    let prefix = package.tag_prefix(&config.tag_prefix);
    let message = format!("Release {version}");
    if !dry_run {
        vcs.create_tag(&prefix, &version, &message)?;
    }
    Ok(Some(format!("{prefix}{version}")))
}

/// The shared body of [`release`] and [`candidate`].
fn run(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
    mode: Mode,
    dry_run: bool,
) -> Result<Option<Outcome>> {
    let changes = recording::unreleased(repo_root, config, package)?;
    let prefix = package.tag_prefix(&config.tag_prefix);
    let package_dir = package.dir(repo_root);
    let current = source(&package_dir, &prefix, vcs).current()?;
    let Some(target) = release_version(&changes, &current) else {
        return Ok(None);
    };

    let mut changelog = read_changelog(repo_root, config, package)?;
    // In fragments mode the Changelog holds releases only. A hand-written
    // Unreleased section would be dropped without ever being promoted, so
    // refuse rather than silently discard it.
    if config.recording == Recording::Fragments && changelog.has_unreleased() {
        let path = package.changelog(repo_root, config);
        bail!(
            "{} holds an Unreleased section, but this package records changes as fragments; \
             move those entries into {} as fragments, or set recording = \"changelog\"",
            path.display(),
            config.changes_dir.display(),
        );
    }
    // A configured `remote` wins; otherwise link to the remote the VCS has.
    let derived = if config.remote.is_some() {
        None
    } else {
        derive_remote(vcs)?
    };
    let links = Links {
        prefix: &prefix,
        remote: config.remote.as_ref().or(derived.as_ref()),
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

    let changelog_path = package.changelog(repo_root, config);
    if !dry_run {
        fs::write(&changelog_path, changelog.render())
            .with_context(|| format!("writing {}", changelog_path.display()))?;
    }

    let consumed = match mode {
        Mode::Release => recording::consume(repo_root, config, package, dry_run)?,
        Mode::Candidate => Vec::new(),
    };

    // Both verbs advance the version source. A Candidate has to be taggable —
    // that is the point of cutting one — and the eventual Release reads the
    // Candidate's version back and drops the `-rc.N` suffix.
    if !dry_run {
        FileVersionSource::new(&package_dir).write(&version)?;
    }
    let version_file = package.version_file(repo_root);

    // The ecosystem manifest follows the version file. It is a Release step, so
    // a `--dry-run` writes nothing and a Package with no manifest is skipped.
    let manifest = sync_manifest(&package_dir, package, &version, dry_run)?;

    Ok(Some(Outcome {
        version,
        changelog: changelog_path,
        version_file,
        consumed,
        manifest,
    }))
}

/// Write `version` into the Package's ecosystem manifest, when it has one.
///
/// A Release step only: `--dry-run` writes nothing. A Package whose ecosystem
/// has no adapter, or that has no manifest on disk, is skipped — the Release
/// still succeeds. The manifest's lockfile is reported, never rewritten.
fn sync_manifest(
    package_dir: &Path,
    package: &Package,
    version: &Version,
    dry_run: bool,
) -> Result<Option<ManifestUpdate>> {
    let Some(sync) = crate::adapters::manifest::for_package(package) else {
        return Ok(None);
    };
    let Some(manifest) = sync.locate(package_dir, package.manifest_override()) else {
        return Ok(None);
    };
    if dry_run {
        return Ok(None);
    }
    Ok(sync.write(&manifest, version)?)
}

/// The Changelog on disk, or a new one when the file is missing.
fn read_changelog(repo_root: &Path, config: &Config, package: &Package) -> Result<Changelog> {
    let path = package.changelog(repo_root, config);
    match fs::read_to_string(&path) {
        Ok(text) => Ok(Changelog::parse(&text)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(Changelog::empty()),
        Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
    }
}

/// The Remote derived from the VCS, when it has one momo recognises.
///
/// A VCS remote momo cannot understand is ignored with a warning: it cannot be
/// linked to, and there is nothing for the user to fix to keep releasing. A
/// configured `remote` never reaches here — config resolution has already
/// rejected an unusable one.
fn derive_remote(vcs: Option<&dyn Vcs>) -> Result<Option<Remote>> {
    let Some(vcs) = vcs else {
        return Ok(None);
    };
    let Some(url) = vcs.remote()? else {
        return Ok(None);
    };
    match url.parse() {
        Ok(remote) => Ok(Some(remote)),
        Err(_) => {
            eprintln!("warning: remote `{url}` is not recognised; no reference links added");
            Ok(None)
        }
    }
}

/// The Release Candidate suffix template from the configuration.
fn rc_format(config: &Config) -> &str {
    config.rc_format.as_deref().unwrap_or(DEFAULT_RC_FORMAT)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    use crate::domain::config::Recording;
    use crate::domain::vcs::{VcsError, VcsKind};
    use tempfile::TempDir;

    use crate::cli::version::VERSION_FILE;

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

    /// The single Package every test releases.
    fn package() -> Package {
        Package::single("momo")
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

    /// Seed one Fragment under the Package's changes directory.
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

        let outcome = release(root, &fragments(), &package(), None, date(), false)
            .expect("release")
            .expect("something to release");

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

        release(root, &fragments(), &package(), None, date(), false).expect("release");

        assert_eq!(read(root, ".momo/version"), "0.1.0\n");
    }

    #[test]
    fn a_fragments_release_refuses_a_changelog_resident_unreleased_section() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        let seeded = "# Changelog\n\n## [Unreleased]\n\n### Fixed\n\n- a hand-written fix\n";
        fs::write(root.join("CHANGELOG.md"), seeded).expect("seed the changelog");

        let err = release(root, &fragments(), &package(), None, date(), false)
            .expect_err("a mixed recording state is refused");
        let message = err.to_string();
        assert!(message.contains("CHANGELOG.md"), "{message}");
        assert!(message.contains("fragments"), "{message}");

        // Nothing was written, and the fragment was not consumed.
        assert_eq!(read(root, "CHANGELOG.md"), seeded);
        assert!(root.join(".momo/changes/add-a-thing.md").exists());
        assert!(!root.join(".momo/version").exists());
    }

    #[test]
    fn a_fragments_release_ignores_an_empty_changelog_unreleased_heading() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        fs::write(
            root.join("CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n",
        )
        .expect("seed the changelog");

        release(root, &fragments(), &package(), None, date(), false)
            .expect("release")
            .expect("something to release");

        let changelog = read(root, "CHANGELOG.md");
        assert!(changelog.contains("## [0.1.0]"), "{changelog}");
        assert!(changelog.contains("- Add a thing"), "{changelog}");
    }

    #[test]
    fn a_multiline_fragment_is_collapsed_into_one_entry() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(
            root,
            "add-a-thing.md",
            "Add a thing\n## [9.9.9] - 2000-01-01",
        );

        release(root, &fragments(), &package(), None, date(), false)
            .expect("release")
            .expect("something to release");

        let changelog = read(root, "CHANGELOG.md");
        assert!(
            changelog.contains("- Add a thing ## [9.9.9] - 2000-01-01"),
            "{changelog}"
        );
        assert!(
            !changelog.lines().any(|line| line.starts_with("## [9.9.9]")),
            "{changelog}"
        );
    }

    #[test]
    fn an_empty_fragment_records_no_change() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-blank.md", "   ");

        let outcome = release(root, &fragments(), &package(), None, date(), false)
            .expect("an empty fragment is not an error");
        assert!(outcome.is_none());
    }

    #[test]
    fn a_release_with_nothing_unreleased_is_none() {
        let dir = TempDir::new().expect("temp dir");

        let outcome = release(
            dir.path(),
            &Config::default(),
            &package(),
            None,
            date(),
            false,
        )
        .expect("an empty release is not an error");
        assert!(outcome.is_none());
    }

    #[test]
    fn a_release_takes_its_version_from_the_tag_when_a_vcs_is_present() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-more.md", "More");
        let vcs = FakeVcs::new().latest(Version::new(0, 1, 0));

        let outcome = release(root, &fragments(), &package(), Some(&vcs), date(), false)
            .expect("release")
            .expect("something to release");

        assert_eq!(outcome.version, Version::new(0, 2, 0));
    }

    #[test]
    fn candidates_increment_and_restart_when_the_target_changes() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        let config = fragments();
        fragment(root, "add-a-thing.md", "Add a thing");

        let first = candidate(root, &config, &package(), None, date(), false)
            .expect("first candidate")
            .expect("something to cut");
        assert_eq!(first.version.to_string(), "0.1.0-rc.1");

        let second = candidate(root, &config, &package(), None, date(), false)
            .expect("second candidate")
            .expect("something to cut");
        assert_eq!(second.version.to_string(), "0.1.0-rc.2");

        // A Release drops the suffix and consumes the shared Fragments.
        let released = release(root, &config, &package(), None, date(), false)
            .expect("release")
            .expect("something to release");
        assert_eq!(released.version.to_string(), "0.1.0");

        // A new Change targets 0.2.0, so the counter restarts at 1.
        fragment(root, "add-another.md", "Another thing");
        let third = candidate(root, &config, &package(), None, date(), false)
            .expect("third candidate")
            .expect("something to cut");
        assert_eq!(third.version.to_string(), "0.2.0-rc.1");
    }

    #[test]
    fn a_candidate_shares_the_fragments_with_the_eventual_release() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");

        let outcome = candidate(root, &fragments(), &package(), None, date(), false)
            .expect("candidate")
            .expect("something to cut");

        assert!(outcome.consumed.is_empty());
        assert!(root.join(".momo/changes/add-a-thing.md").exists());
        // The Candidate still advances the version source, so `tag` can name it.
        let version_file = root.join(VERSION_FILE);
        assert_eq!(outcome.version_file, version_file);
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

        release(root, &fragments(), &package(), Some(&vcs), date(), false).expect("release");

        let changelog = read(root, "CHANGELOG.md");
        assert!(
            changelog.contains("[0.1.0]: https://github.com/Jarvvski/momo/releases/tag/v0.1.0"),
            "{changelog}"
        );
        // The release consumed the Unreleased section, so the footer must not
        // define a link to it.
        assert!(!changelog.contains("[Unreleased]:"), "{changelog}");
    }

    #[test]
    fn a_release_with_an_unrecognised_vcs_remote_still_releases_without_links() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        let vcs = FakeVcs::new().remote("not a remote");

        let outcome = release(root, &fragments(), &package(), Some(&vcs), date(), false)
            .expect("an unrecognised remote is a warning, not a failure")
            .expect("something to release");

        assert_eq!(outcome.version, Version::new(0, 1, 0));
        let changelog = read(root, "CHANGELOG.md");
        assert!(changelog.contains("## [0.1.0]"), "{changelog}");
        assert!(!changelog.contains("[0.1.0]:"), "{changelog}");
    }

    #[test]
    fn a_candidate_keeps_the_unreleased_reference_link() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fs::write(
            root.join("CHANGELOG.md"),
            "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- Add a thing\n",
        )
        .expect("seed the changelog");
        let vcs = FakeVcs::new().remote("git@github.com:Jarvvski/momo.git");

        candidate(
            root,
            &Config::default(),
            &package(),
            Some(&vcs),
            date(),
            false,
        )
        .expect("candidate");

        let changelog = read(root, "CHANGELOG.md");
        assert!(changelog.contains("## [Unreleased]"), "{changelog}");
        assert!(
            changelog.contains(
                "[Unreleased]: https://github.com/Jarvvski/momo/compare/v0.1.0-rc.1...HEAD"
            ),
            "{changelog}"
        );
        assert!(
            changelog.contains(
                "[0.1.0-rc.1]: https://github.com/Jarvvski/momo/releases/tag/v0.1.0-rc.1"
            ),
            "{changelog}"
        );
    }

    #[test]
    fn a_configured_remote_overrides_the_vcs_remote() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        let mut config = fragments();
        config.remote = Some("https://gitlab.com/a/b".parse().expect("a remote URL"));
        let vcs = FakeVcs::new().remote("git@github.com:Jarvvski/momo.git");

        release(root, &config, &package(), Some(&vcs), date(), false).expect("release");

        let changelog = read(root, "CHANGELOG.md");
        assert!(
            changelog.contains("[0.1.0]: https://gitlab.com/a/b/-/tags/v0.1.0"),
            "{changelog}"
        );
        assert!(
            !changelog.contains("github.com/Jarvvski/momo/releases"),
            "{changelog}"
        );
    }

    #[test]
    fn a_configured_remote_supplies_one_without_a_vcs() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        let mut config = fragments();
        config.remote = Some(
            "git@github.com:Jarvvski/momo.git"
                .parse()
                .expect("a remote URL"),
        );

        release(root, &config, &package(), None, date(), false).expect("release");

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

        let outcome = release(root, &fragments(), &package(), None, date(), true)
            .expect("dry-run release")
            .expect("something to release");

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

        let name = tag(root, &Config::default(), &package(), &FakeVcs::new(), false)
            .expect("tag")
            .expect("something to tag");

        assert_eq!(name, "v1.2.3");
    }

    #[test]
    fn a_tag_names_the_version_a_candidate_minted() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");

        let outcome = candidate(root, &fragments(), &package(), None, date(), false)
            .expect("candidate")
            .expect("something to cut");
        assert_eq!(outcome.version.to_string(), "0.1.0-rc.1");

        let name = tag(root, &fragments(), &package(), &FakeVcs::new(), false)
            .expect("tag")
            .expect("something to tag");

        assert_eq!(name, "v0.1.0-rc.1");
    }

    #[test]
    fn a_tag_without_a_release_is_none() {
        let dir = TempDir::new().expect("temp dir");

        let name = tag(
            dir.path(),
            &Config::default(),
            &package(),
            &FakeVcs::new(),
            false,
        )
        .expect("tagging with nothing released is not an error");
        assert!(name.is_none());
    }

    #[test]
    fn a_monorepo_release_writes_the_package_changelog_and_namespaced_tag() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        let package = Package::from_identity("cargo:a", "crates/a");
        fs::create_dir_all(root.join("crates/a")).expect("package dir");
        fs::create_dir_all(root.join(".momo/changes/cargo-a")).expect("package changes dir");
        fs::write(
            root.join(".momo/changes/cargo-a/add-a-thing.md"),
            "Add a thing\n",
        )
        .expect("write a fragment");

        release(root, &fragments(), &package, None, date(), false)
            .expect("release")
            .expect("something to release");

        assert!(root.join("crates/a/CHANGELOG.md").exists());
        assert_eq!(read(root, "crates/a/.momo/version"), "0.1.0\n");
        assert!(
            !root.join("CHANGELOG.md").exists(),
            "a monorepo release wrote a repo-root changelog"
        );

        let name = tag(root, &fragments(), &package, &FakeVcs::new(), false)
            .expect("tag")
            .expect("something to tag");
        assert_eq!(name, "cargo-a-v0.1.0");
    }

    /// Seed a Cargo manifest at `path` with the bootstrap version.
    fn manifest(root: &Path, path: &str) -> String {
        let contents = "[package]\nname = \"momo\"\nversion = \"0.0.0\"\n";
        let full = root.join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("create the manifest dir");
        }
        fs::write(&full, contents).expect("write the manifest");
        contents.to_owned()
    }

    #[test]
    fn a_release_syncs_the_ecosystem_manifest() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        manifest(root, "Cargo.toml");

        let outcome = release(root, &fragments(), &package(), None, date(), false)
            .expect("release")
            .expect("something to release");

        assert_eq!(outcome.version, Version::new(0, 1, 0));
        let update = outcome.manifest.as_ref().expect("a manifest was synced");
        assert_eq!(update.manifest, root.join("Cargo.toml"));
        assert_eq!(
            read(root, "Cargo.toml"),
            "[package]\nname = \"momo\"\nversion = \"0.1.0\"\n"
        );
    }

    #[test]
    fn a_release_without_a_manifest_skips_sync() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");

        let outcome = release(root, &fragments(), &package(), None, date(), false)
            .expect("release")
            .expect("something to release");

        assert!(outcome.manifest.is_none());
    }

    #[test]
    fn a_dry_run_release_writes_no_manifest() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        let seeded = manifest(root, "Cargo.toml");

        let outcome = release(root, &fragments(), &package(), None, date(), true)
            .expect("dry-run release")
            .expect("something to release");

        assert!(outcome.manifest.is_none());
        assert_eq!(read(root, "Cargo.toml"), seeded);
    }

    #[test]
    fn a_monorepo_release_syncs_the_package_manifest() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        let package = Package::from_identity("cargo:a", "crates/a");
        fs::create_dir_all(root.join("crates/a")).expect("package dir");
        fs::create_dir_all(root.join(".momo/changes/cargo-a")).expect("package changes dir");
        fs::write(
            root.join(".momo/changes/cargo-a/add-a-thing.md"),
            "Add a thing\n",
        )
        .expect("write a fragment");
        manifest(root, "crates/a/Cargo.toml");

        release(root, &fragments(), &package, None, date(), false)
            .expect("release")
            .expect("something to release");

        assert_eq!(
            read(root, "crates/a/Cargo.toml"),
            "[package]\nname = \"momo\"\nversion = \"0.1.0\"\n"
        );
    }

    #[test]
    fn a_configured_manifest_override_wins_over_discovery() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        let conventional = manifest(root, "Cargo.toml");
        manifest(root, "alt.toml");

        let mut config = fragments();
        config.packages.insert(
            "momo".to_owned(),
            crate::domain::config::PackageConfig {
                manifest: Some(PathBuf::from("alt.toml")),
                ..crate::domain::config::PackageConfig::default()
            },
        );

        let package = package().configured(&config);
        release(root, &config, &package, None, date(), false)
            .expect("release")
            .expect("something to release");

        assert_eq!(
            read(root, "alt.toml"),
            "[package]\nname = \"momo\"\nversion = \"0.1.0\"\n"
        );
        assert_eq!(read(root, "Cargo.toml"), conventional);
    }
}
