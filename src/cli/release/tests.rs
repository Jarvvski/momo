use super::*;

use std::path::Path;

use crate::domain::config::Recording;
use crate::domain::manifest::SkipReason;
use crate::domain::vcs::{Vcs, VcsError, VcsKind};
use tempfile::TempDir;
use time::Date;

use crate::cli::version::{FileVersionSource, VERSION_FILE};

/// Resolve and write a Release in one step, as a single-Package caller does.
fn release(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
    dry_run: bool,
) -> Result<Option<Outcome>> {
    let Some(prepared) = prepare_release(repo_root, config, package, vcs, date)? else {
        return Ok(None);
    };
    commit(prepared, dry_run)
}

/// Resolve and write a Release Candidate in one step.
fn candidate(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
    dry_run: bool,
) -> Result<Option<Outcome>> {
    let Some(prepared) = prepare_candidate(repo_root, config, package, vcs, date)? else {
        return Ok(None);
    };
    commit(prepared, dry_run)
}

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
        changelog
            .contains("[Unreleased]: https://github.com/Jarvvski/momo/compare/v0.1.0-rc.1...HEAD"),
        "{changelog}"
    );
    assert!(
        changelog
            .contains("[0.1.0-rc.1]: https://github.com/Jarvvski/momo/releases/tag/v0.1.0-rc.1"),
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

#[test]
fn a_go_module_is_tagged_by_its_directory_not_its_slug() {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path();
    let package = Package::from_identity("go:example.com/acme/api", "api");
    fs::create_dir_all(root.join("api")).expect("package dir");
    fs::create_dir_all(root.join(".momo/changes/go-example-com-acme-api"))
        .expect("package changes dir");
    fs::write(
        root.join(".momo/changes/go-example-com-acme-api/add-a-thing.md"),
        "Add a thing\n",
    )
    .expect("write a fragment");

    release(root, &fragments(), &package, None, date(), false)
        .expect("release")
        .expect("something to release");

    // Go resolves a module version by its directory, so the tag is `api/v…`,
    // not the slug-namespaced `go-example-com-acme-api-v…`.
    let name = tag(root, &fragments(), &package, &FakeVcs::new(), None, false)
        .expect("tag")
        .expect("something to tag");
    assert_eq!(name.name, "api/v0.1.0");
}

#[test]
fn a_configured_tag_prefix_overrides_the_derivation_in_the_tag() {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path();
    let mut config = fragments();
    config.packages.insert(
        "go:example.com/acme/api".to_owned(),
        crate::domain::config::PackageConfig {
            tag_prefix: Some("api/v".to_owned()),
            ..crate::domain::config::PackageConfig::default()
        },
    );
    let package = Package::from_identity("go:example.com/acme/api", "api").configured(&config);
    fs::create_dir_all(root.join("api")).expect("package dir");
    fs::create_dir_all(root.join(".momo/changes/go-example-com-acme-api"))
        .expect("package changes dir");
    fs::write(
        root.join(".momo/changes/go-example-com-acme-api/add-a-thing.md"),
        "Add a thing\n",
    )
    .expect("write a fragment");

    release(root, &config, &package, None, date(), false)
        .expect("release")
        .expect("something to release");

    let name = tag(root, &config, &package, &FakeVcs::new(), None, false)
        .expect("tag")
        .expect("something to tag");
    assert_eq!(name.name, "api/v0.1.0");
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
    let plan = outcome.manifest.as_ref().expect("a manifest was synced");
    assert_eq!(plan.writes.len(), 1, "{plan:?}");
    assert_eq!(plan.writes[0].path, root.join("Cargo.toml"));
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
fn a_manifest_without_a_version_is_reported_as_skipped() {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path();
    fragment(root, "add-a-thing.md", "Add a thing");
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"momo\"\n")
        .expect("seed a versionless manifest");

    let outcome = release(root, &fragments(), &package(), None, date(), false)
        .expect("release")
        .expect("something to release");

    let plan = outcome
        .manifest
        .as_ref()
        .expect("the manifest was considered");
    assert!(plan.writes.is_empty(), "{plan:?}");
    assert_eq!(
        plan.skipped,
        vec![(root.join("Cargo.toml"), SkipReason::NoVersion)],
        "{plan:?}"
    );
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

    let plan = outcome
        .manifest
        .as_ref()
        .expect("the manifest is reported under --dry-run");
    assert_eq!(plan.writes[0].path, root.join("Cargo.toml"));
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

/// Seed one member of the workspace rooted at `root`.
fn member(root: &Path, name: &str, fragment: &str, text: &str) {
    let package = root.join("crates").join(name);
    fs::create_dir_all(&package).expect("package dir");
    fs::write(
        package.join("Cargo.toml"),
        format!("[package]\nname = \"{name}\"\nversion.workspace = true\n"),
    )
    .expect("seed the member manifest");
    let changes = root.join(".momo/changes").join(format!("cargo-{name}"));
    fs::create_dir_all(&changes).expect("package changes dir");
    fs::write(changes.join(fragment), format!("{text}\n")).expect("write a fragment");
}

/// A workspace root whose members inherit `[workspace.package].version`.
fn workspace_root(root: &Path) -> String {
    let text = "[workspace]\nmembers = [\"crates/a\", \"crates/b\"]\n\n\
                [workspace.package]\nversion = \"0.0.0\"\n";
    fs::write(root.join("Cargo.toml"), text).expect("seed the workspace root");
    text.to_owned()
}

#[test]
fn a_monorepo_release_rewrites_the_shared_workspace_version() {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path();
    let package = Package::from_identity("cargo:a", "crates/a");
    let seeded = workspace_root(root);
    member(root, "a", "add-a-thing.md", "Add a thing");

    let outcome = release(root, &fragments(), &package, None, date(), false)
        .expect("release")
        .expect("something to release");

    assert_eq!(outcome.version, Version::new(0, 1, 0));
    let plan = outcome.manifest.as_ref().expect("a manifest was synced");
    assert_eq!(plan.writes[0].path, root.join("Cargo.toml"));
    assert_eq!(
        read(root, "Cargo.toml"),
        seeded.replace("version = \"0.0.0\"", "version = \"0.1.0\""),
        "the shared workspace version was not rewritten"
    );
    assert_eq!(
        read(root, "crates/a/Cargo.toml"),
        "[package]\nname = \"a\"\nversion.workspace = true\n",
        "the member's own manifest was rewritten"
    );
}

#[test]
fn two_members_sharing_a_workspace_version_must_agree_on_the_target() {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path();
    let seeded = workspace_root(root);
    member(root, "a", "add-a-thing.md", "Add a thing");
    member(root, "b", "fix-a-thing.md", "Fix a thing");
    let a = Package::from_identity("cargo:a", "crates/a");
    let b = Package::from_identity("cargo:b", "crates/b");

    let plans = [
        (
            &a,
            prepare_release(root, &fragments(), &a, None, date())
                .expect("prepare a")
                .expect("a release"),
        ),
        (
            &b,
            prepare_release(root, &fragments(), &b, None, date())
                .expect("prepare b")
                .expect("a release"),
        ),
    ];
    assert_eq!(plans[0].1.version(), &Version::new(0, 1, 0));
    assert_eq!(plans[1].1.version(), &Version::new(0, 0, 1));

    let err = reject_shared_manifest_conflicts(&plans)
        .expect_err("the two targets cannot share one scalar");
    let message = err.to_string();
    assert!(message.contains("cargo:a"), "{message}");
    assert!(message.contains("cargo:b"), "{message}");
    assert!(
        message.contains("0.1.0") && message.contains("0.0.1"),
        "{message}"
    );

    // Preparation is read-only, so the refusal left the tree untouched.
    assert_eq!(read(root, "Cargo.toml"), seeded);
}

#[test]
fn two_members_sharing_a_workspace_version_may_agree_on_the_target() {
    let dir = TempDir::new().expect("temp dir");
    let root = dir.path();
    workspace_root(root);
    member(root, "a", "add-a-thing.md", "Add a thing");
    member(root, "b", "add-another.md", "Add another");
    let a = Package::from_identity("cargo:a", "crates/a");
    let b = Package::from_identity("cargo:b", "crates/b");

    let plans = [
        (
            &a,
            prepare_release(root, &fragments(), &a, None, date())
                .expect("prepare a")
                .expect("a release"),
        ),
        (
            &b,
            prepare_release(root, &fragments(), &b, None, date())
                .expect("prepare b")
                .expect("a release"),
        ),
    ];

    reject_shared_manifest_conflicts(&plans).expect("one shared target is allowed");
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
