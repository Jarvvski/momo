//! The Release lifecycle: minting a Version, promoting the Changelog, consuming
//! Fragments, and creating the VCS tag.
//!
//! Everything here is orchestration, per Package. The pure decisions — the Bump,
//! the Changelog text, the reference links — live in `momo-core`; the file work
//! goes through [`crate::recording`] and [`FileVersionSource`], and the
//! repository is touched only through the injected [`Vcs`].

use std::cmp::Ordering;
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
    DEFAULT_RC_FORMAT, VersionSource, candidate_version, next_candidate, release_target,
};
use anyhow::{Context, Result, bail};
use semver::Version;
use time::Date;

use crate::cli::atomic;
use crate::cli::recording;
use crate::cli::version::{FileVersionSource, release_base, source};

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

/// A tag [`tag`] created, or would create.
#[derive(Debug)]
pub struct Tagged {
    /// The Version the tag names.
    pub version: Version,
    /// The full tag name, prefix included.
    pub name: String,
}

/// Create the VCS tag for the Version the ledger records.
///
/// The tag names the Version the last `release`/`candidate` minted — the one no
/// tag records yet — so `tag` brings the repository and momo's own record into
/// sync. A ledger that is *older* than the newest tag is stale: a later Version
/// is already tagged, so tagging this one would move the release backwards. It
/// is refused rather than created. `rev` overrides the adapter's default anchor
/// (jj: `@-`, git: `HEAD`). Returns the tag, or `None` when the Package has
/// never minted a Version.
pub fn tag(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: &dyn Vcs,
    rev: Option<&str>,
    dry_run: bool,
) -> Result<Option<Tagged>> {
    let Some(version) = FileVersionSource::new(&package.dir(repo_root)).current()? else {
        return Ok(None);
    };
    let prefix = package.tag_prefix(&config.tag_prefix);

    // The ledger must name the pending Version: one no tag records yet. A
    // Version below the newest tag is stale — a later one is already tagged, so
    // tagging this one would publish a release behind it.
    if let Some(latest) = vcs.latest_tag(&prefix)?
        && version.cmp_precedence(&latest) == Ordering::Less
    {
        bail!(
            "the version file holds `{version}`, which is older than the newest tag `{latest}`; \
             run `momo release` to advance it before tagging"
        );
    }

    let message = format!("Release {version}");
    if !dry_run {
        vcs.create_tag(rev, &prefix, &version, &message)?;
    }
    Ok(Some(Tagged {
        name: format!("{prefix}{version}"),
        version,
    }))
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
    let mut changelog = read_changelog(repo_root, config, package)?;

    // The base is the last *release*: a Candidate cut since it does not move the
    // target, and recomputing from the Candidate would double-count the bump.
    // The version source reports it from the tags; a VCS-less Package whose
    // ledger holds a Candidate falls back to the newest Release the Changelog
    // records.
    let version_source = source(&package_dir, &prefix, vcs);
    let last_release = release_base(version_source.as_ref(), &changelog)?;
    let current = version_source.current()?;
    let candidate = current.as_ref().filter(|version| !version.pre.is_empty());
    let Some(target) = release_target(&changes, &last_release, candidate)? else {
        return Ok(None);
    };

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
            let n = next_candidate(&target, format, recorded.iter().chain(current.iter()))?;
            let version = candidate_version(&target, n, format)?;
            changelog.candidate(&version, date, &changes, links);
            version
        }
    };

    let changelog_path = package.changelog(repo_root, config);
    let version_file = package.version_file(repo_root);

    // Validate before mutating. Rendering the manifest parses it — the step
    // that can fail on a malformed manifest — so it runs here, before a single
    // target is touched. The Fragments are collected, not deleted.
    let manifest = render_manifest(&package_dir, package, &version)?;
    let consumed = match mode {
        Mode::Release => recording::fragments(repo_root, config, package)?,
        Mode::Candidate => Vec::new(),
    };

    let outcome = Outcome {
        version,
        changelog: changelog_path.clone(),
        version_file: version_file.clone(),
        consumed,
        manifest: manifest.as_ref().map(|(_, update)| update.clone()),
    };
    let version_text = format!("{}\n", outcome.version);

    // Stage every artefact beside its own target. Any failure here — a
    // read-only directory, a full disk — leaves the tree exactly as it was.
    let mut writes: Vec<(&Path, String)> = vec![
        (changelog_path.as_path(), changelog.render()),
        (version_file.as_path(), version_text),
    ];
    if let Some((text, update)) = &manifest {
        writes.push((update.manifest.as_path(), text.clone()));
    }

    let mut staged = Staged::default();
    for (target, contents) in &writes {
        if let Err(err) = staged.add(target, contents) {
            staged.discard();
            return Err(err);
        }
    }

    if dry_run {
        staged.discard();
        return Ok(Some(outcome));
    }

    // Commit: rename every staged file over its target, then delete the
    // Fragments last, so a retry can still see them if anything goes wrong.
    staged.commit()?;
    recording::consume(&outcome.consumed)?;

    Ok(Some(outcome))
}

/// A Release's rendered artefacts, staged as temporary files until every one is
/// ready to be renamed into place.
///
/// Staging is what makes the transaction all-or-nothing: no target named here
/// changes until [`Staged::commit`], and any failure before it is undone by
/// [`Staged::discard`].
#[derive(Default)]
struct Staged {
    /// Each staged write: its temporary file and the target it will replace.
    writes: Vec<(PathBuf, PathBuf)>,
    /// Directories staging created, removed again on discard when still empty.
    created: Vec<PathBuf>,
}

impl Staged {
    /// Stage `contents` for `target`, creating its directory when needed.
    fn add(&mut self, target: &Path, contents: &str) -> Result<()> {
        if let Some(parent) = target.parent()
            && !parent.as_os_str().is_empty()
            && !parent.exists()
        {
            fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
            self.created.push(parent.to_path_buf());
        }
        let temp = atomic::stage(target, contents)
            .with_context(|| format!("writing {}", target.display()))?;
        self.writes.push((temp, target.to_path_buf()));
        Ok(())
    }

    /// Rename every staged file over its target, in order.
    ///
    /// A failure removes the temporary files that have not been renamed yet; the
    /// targets already renamed stay committed.
    fn commit(self) -> Result<()> {
        for (index, (temp, target)) in self.writes.iter().enumerate() {
            if let Err(err) = fs::rename(temp, target) {
                for (temp, _) in &self.writes[index..] {
                    let _ = fs::remove_file(temp);
                }
                return Err(err).with_context(|| format!("replacing {}", target.display()));
            }
        }
        Ok(())
    }

    /// Remove every staged file, leaving the tree as it was.
    fn discard(self) {
        for (temp, _) in &self.writes {
            let _ = fs::remove_file(temp);
        }
        for dir in &self.created {
            let _ = fs::remove_dir(dir);
        }
    }
}

/// Render `version` into the Package's ecosystem manifest, when it has one.
///
/// A read-only step: it parses the manifest, so a malformed one fails before
/// anything is written, and returns the text to stage. A Package whose
/// ecosystem has no adapter, or that has no manifest on disk, is skipped — the
/// Release still succeeds. An inherited version momo cannot resolve is an error,
/// never a silent skip.
fn render_manifest(
    package_dir: &Path,
    package: &Package,
    version: &Version,
) -> Result<Option<(String, ManifestUpdate)>> {
    let Some(sync) = crate::adapters::manifest::for_package(package) else {
        return Ok(None);
    };
    let Some(manifest) = sync.locate(package_dir, package.manifest_override()) else {
        return Ok(None);
    };
    Ok(sync.render(&manifest, version)?)
}

/// The Changelog on disk, or a new one when the file is missing.
pub(crate) fn read_changelog(
    repo_root: &Path,
    config: &Config,
    package: &Package,
) -> Result<Changelog> {
    let path = package.changelog(repo_root, config);
    match fs::read_to_string(&path) {
        Ok(text) => Changelog::parse(&text).with_context(|| format!("reading {}", path.display())),
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
        release: Option<Version>,
        remote: Option<String>,
    }

    impl FakeVcs {
        fn new() -> Self {
            Self {
                latest: None,
                release: None,
                remote: None,
            }
        }

        /// Record `version` as the highest tag; a stable Version is also the
        /// highest Release tag.
        fn latest(mut self, version: Version) -> Self {
            self.release = version.pre.is_empty().then(|| version.clone());
            self.latest = Some(version);
            self
        }

        /// Record `version` as the highest Release tag, below a Candidate.
        fn release(mut self, version: Version) -> Self {
            self.release = Some(version);
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

        fn latest_release_tag(&self, _prefix: &str) -> Result<Option<Version>, VcsError> {
            Ok(self.release.clone())
        }

        fn remote(&self) -> Result<Option<String>, VcsError> {
            Ok(self.remote.clone())
        }

        fn create_tag(
            &self,
            _rev: Option<&str>,
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
    fn a_release_recomputes_from_the_last_release_not_the_candidate() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-add-a-thing.md", "Add a thing");
        // Released `1.0.0`, with a `1.1.0-rc.1` cut from it and the same Add
        // still open.
        let vcs = FakeVcs::new()
            .latest(Version::parse("1.1.0-rc.1").expect("a valid version"))
            .release(Version::new(1, 0, 0));

        let outcome = release(root, &fragments(), &package(), Some(&vcs), date(), false)
            .expect("release")
            .expect("something to release");

        // Not `1.2.0`: the bump applies to `1.0.0`, not to the candidate's
        // `1.1.0` — recomputing from the candidate would double-count the bump.
        assert_eq!(outcome.version, Version::new(1, 1, 0));
    }

    #[test]
    fn a_release_after_a_candidate_is_refused_when_a_break_moves_the_target() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-add-a-thing.md", "Add a thing");
        fragment(root, "break-break-a-thing.md", "Break a thing");
        // Released `1.0.0`, with a `1.1.0-rc.1` cut from it; the Break now
        // implies `2.0.0`, so the cut candidate is stale.
        let vcs = FakeVcs::new()
            .latest(Version::parse("1.1.0-rc.1").expect("a valid version"))
            .release(Version::new(1, 0, 0));

        let err = release(root, &fragments(), &package(), Some(&vcs), date(), false)
            .expect_err("the cut candidate is stale");
        let message = err.to_string();
        assert!(message.contains("1.1.0-rc.1"), "{message}");
        assert!(message.contains("2.0.0"), "{message}");
        assert!(message.contains("cut a new candidate"), "{message}");

        // Nothing was promoted, so the tree is ready for a fresh candidate.
        assert!(!root.join("CHANGELOG.md").exists());
        assert!(!root.join(".momo/version").exists());
    }

    #[test]
    fn a_candidate_after_a_candidate_at_major_one_is_refused_when_a_break_lands() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-add-a-thing.md", "Add a thing");
        fragment(root, "break-break-a-thing.md", "Break a thing");
        let vcs = FakeVcs::new()
            .latest(Version::parse("1.1.0-rc.1").expect("a valid version"))
            .release(Version::new(1, 0, 0));

        let err = candidate(root, &fragments(), &package(), Some(&vcs), date(), false)
            .expect_err("the cut candidate is stale");
        assert!(err.to_string().contains("cut a new candidate"), "{err}");
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
    fn a_dry_run_release_leaves_no_new_directory_behind() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        // Changelog mode: nothing pre-creates `.momo`, so the version file's
        // directory is the only thing staging would add.
        let seeded = "# Changelog\n\n## [Unreleased]\n\n### Added\n\n- Add a thing\n";
        fs::write(root.join("CHANGELOG.md"), seeded).expect("seed the changelog");

        release(root, &Config::default(), &package(), None, date(), true)
            .expect("dry-run release")
            .expect("something to release");

        assert!(
            !root.join(".momo").exists(),
            "the dry run left a `.momo` directory behind"
        );
        assert_eq!(read(root, "CHANGELOG.md"), seeded);
    }

    #[test]
    fn a_tag_names_the_version_momo_last_released() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        FileVersionSource::new(root)
            .write(&Version::new(1, 2, 3))
            .expect("write the version file");

        let tagged = tag(
            root,
            &Config::default(),
            &package(),
            &FakeVcs::new(),
            None,
            false,
        )
        .expect("tag")
        .expect("something to tag");

        assert_eq!(tagged.name, "v1.2.3");
        assert_eq!(tagged.version, Version::new(1, 2, 3));
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

        let tagged = tag(root, &fragments(), &package(), &FakeVcs::new(), None, false)
            .expect("tag")
            .expect("something to tag");

        assert_eq!(tagged.name, "v0.1.0-rc.1");
    }

    #[test]
    fn a_tag_without_a_release_is_none() {
        let dir = TempDir::new().expect("temp dir");

        let name = tag(
            dir.path(),
            &Config::default(),
            &package(),
            &FakeVcs::new(),
            None,
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

        let name = tag(root, &fragments(), &package, &FakeVcs::new(), None, false)
            .expect("tag")
            .expect("something to tag");
        assert_eq!(name.name, "cargo-a-v0.1.0");
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
    fn a_dry_run_release_reports_the_manifest_without_writing_it() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        let seeded = manifest(root, "Cargo.toml");

        let outcome = release(root, &fragments(), &package(), None, date(), true)
            .expect("dry-run release")
            .expect("something to release");

        let update = outcome
            .manifest
            .as_ref()
            .expect("the manifest is reported under --dry-run");
        assert_eq!(update.manifest, root.join("Cargo.toml"));
        assert_eq!(
            read(root, "Cargo.toml"),
            seeded,
            "the dry run wrote the manifest"
        );
    }

    /// Every staging file under `root`, recursively.
    fn staging_files(root: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.ends_with(".tmp"))
                {
                    found.push(path);
                }
            }
        }
        found
    }

    #[test]
    fn a_release_leaves_no_staging_files_behind() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        manifest(root, "Cargo.toml");

        release(root, &fragments(), &package(), None, date(), false).expect("release");

        assert_eq!(staging_files(root), Vec::<PathBuf>::new());
    }

    #[test]
    fn an_invalid_manifest_leaves_the_release_unpromoted() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        fs::write(root.join("Cargo.toml"), "version = \n").expect("seed an invalid manifest");

        let err = release(root, &fragments(), &package(), None, date(), false)
            .expect_err("an invalid manifest is an error");
        assert!(err.to_string().contains("Cargo.toml"), "{err}");

        assert!(
            !root.join("CHANGELOG.md").exists(),
            "the changelog was promoted"
        );
        assert!(
            !root.join(".momo/version").exists(),
            "the version file was advanced"
        );
        assert!(
            root.join(".momo/changes/add-a-thing.md").exists(),
            "the fragment was consumed"
        );
        assert_eq!(staging_files(root), Vec::<PathBuf>::new());
    }

    #[test]
    fn a_dry_run_with_an_invalid_manifest_is_an_error() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        fs::write(root.join("Cargo.toml"), "version = \n").expect("seed an invalid manifest");

        let err = release(root, &fragments(), &package(), None, date(), true)
            .expect_err("a dry run must validate the manifest too");
        assert!(err.to_string().contains("Cargo.toml"), "{err}");
        assert!(!root.join("CHANGELOG.md").exists());
    }

    #[test]
    fn an_inherited_manifest_version_is_reported() {
        let dir = TempDir::new().expect("temp dir");
        let root = dir.path();
        fragment(root, "add-a-thing.md", "Add a thing");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"momo\"\nversion.workspace = true\n",
        )
        .expect("seed an inherited manifest");

        let err = release(root, &fragments(), &package(), None, date(), false)
            .expect_err("an unresolvable inherited version is not silent");
        assert!(err.to_string().contains("inherits its version"), "{err}");

        assert!(
            !root.join("CHANGELOG.md").exists(),
            "the changelog was promoted"
        );
        assert!(!root.join(".momo/version").exists());
        assert!(root.join(".momo/changes/add-a-thing.md").exists());
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
