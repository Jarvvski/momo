//! The prepare path and the staged write transaction.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use semver::Version;
use time::Date;

use crate::cli::atomic;
use crate::cli::error::CliError;
use crate::cli::recording;
use crate::cli::version::{release_base, source};
use crate::domain::changelog::{Changelog, Links};
use crate::domain::config::{Config, Recording};
use crate::domain::manifest::{DependencyRangeEdit, ManifestEdits};
use crate::domain::package::Package;
use crate::domain::tag;
use crate::domain::vcs::Vcs;
use crate::domain::version::{
    DEFAULT_RC_FORMAT, candidate_version, next_candidate, release_target,
};

use super::manifest::render_manifest;
use super::tag::derive_remote;
use super::{Outcome, Prepared, read_changelog};

/// Which end of the Release lifecycle a run is minting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// A Release: consume the Unreleased Changes.
    Release,
    /// A Release Candidate: share them with the eventual Release.
    Candidate,
}

/// Resolve a Release Candidate for `package` without writing anything.
pub fn prepare_candidate(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
) -> Result<Option<Prepared>> {
    let Some(resolved) = resolve_candidate(repo_root, config, package, vcs, date)? else {
        return Ok(None);
    };
    Ok(Some(assemble(package, resolved, Vec::new())?))
}

/// Resolve a Release's Version and Changelog, before any manifest is rendered.
///
/// The first pass of a run: a Package's manifest cannot be rendered until every
/// target in the run is known, because the dependency ranges it rewrites are a
/// function of the *other* Packages' Versions. Resolving is read-only, so a run
/// can resolve every Package and still fail before the first byte lands.
pub fn resolve_release(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
) -> Result<Option<Resolved>> {
    resolve(repo_root, config, package, vcs, date, Mode::Release)
}

/// Resolve a Release Candidate's Version and Changelog.
pub fn resolve_candidate(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
) -> Result<Option<Resolved>> {
    resolve(repo_root, config, package, vcs, date, Mode::Candidate)
}

/// Render a [`Resolved`] Release's manifest and assemble its writes.
///
/// The second pass of a run, so the version scalar and the dependency ranges
/// land in one render of one file — see [`render_manifest`]. `ranges` are the
/// edits this Package's own manifest receives.
pub fn assemble(
    package: &Package,
    resolved: Resolved,
    ranges: Vec<DependencyRangeEdit>,
) -> Result<Prepared> {
    let Resolved {
        mode,
        version,
        changelog,
        changelog_path,
        version_file,
        consumed,
        package_dir,
    } = resolved;

    // A Candidate never rewrites a range: `^5.0.0` is not satisfied by
    // `5.1.0-next.0`, so a prerelease would widen every internal range
    // spuriously. The gate lives here, not at the call site, so no caller can
    // slip a Candidate past it.
    let edits = ManifestEdits {
        version: Some(version.clone()),
        ranges: rewritable(mode, ranges),
    };

    // Rendering the manifest parses it — the step that can fail on a malformed
    // manifest — so it runs before a single target is touched. The Fragments
    // were collected, not deleted, when the Release was resolved.
    let manifest = render_manifest(&package_dir, package, &edits)?;
    let outcome = Outcome {
        version,
        changelog: changelog_path.clone(),
        version_file: version_file.clone(),
        consumed,
        manifest,
    };
    let version_text = format!("{}\n", outcome.version);

    // Every artefact, rendered: the Changelog, the version file, and — when the
    // Package has one — each manifest in the plan. An inherited version's
    // manifest is the shared workspace root, outside this Package's directory,
    // but it stages and commits in the same transaction as the rest.
    let mut writes: Vec<(PathBuf, String)> = vec![
        (changelog_path, changelog.render()),
        (version_file, version_text),
    ];
    if let Some(plan) = &outcome.manifest {
        for write in &plan.writes {
            writes.push((write.path.clone(), write.text.clone()));
        }
    }

    Ok(Prepared { outcome, writes })
}

/// The dependency ranges `mode` is allowed to rewrite.
///
/// A Release rewrites the ranges of the dependents in its own set; a Release
/// Candidate leaves every range exactly as the project wrote it.
fn rewritable(mode: Mode, ranges: Vec<DependencyRangeEdit>) -> Vec<DependencyRangeEdit> {
    match mode {
        Mode::Release => ranges,
        Mode::Candidate => Vec::new(),
    }
}

/// Write a [`Prepared`] Release's artefacts, atomically.
///
/// Every artefact is staged beside its own target and renamed into place
/// together, so a failure leaves the tree exactly as it was. `dry_run` stages
/// and discards, validating the run without writing it.
pub fn commit(prepared: Prepared, dry_run: bool) -> Result<Option<Outcome>> {
    let Prepared { outcome, writes } = prepared;

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

/// A Release resolved as far as its Version, before any manifest is rendered.
///
/// Carries the in-memory Changelog the mint produced, so the second pass
/// ([`assemble`]) renders the same document the Version was minted from.
pub struct Resolved {
    /// Which end of the lifecycle was minted.
    mode: Mode,
    /// The Version minted.
    version: Version,
    /// The Changelog with this Release promoted.
    changelog: Changelog,
    /// Where the Changelog is written.
    changelog_path: PathBuf,
    /// The version file the run writes.
    version_file: PathBuf,
    /// The Fragments the run consumed.
    consumed: Vec<PathBuf>,
    /// The Package's directory, where its manifest is located.
    package_dir: PathBuf,
}

impl Resolved {
    /// The Version the run would mint.
    pub fn version(&self) -> &Version {
        &self.version
    }
}

/// The shared body of [`resolve_release`] and [`resolve_candidate`].
fn resolve(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
    mode: Mode,
) -> Result<Option<Resolved>> {
    let changes = recording::unreleased(repo_root, config, package)?;
    let prefix = tag::prefix(package, &config.tag_prefix);
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
        return Err(CliError::UnversionedSection {
            path: package.changelog(repo_root, config),
            changes_dir: config.changes_dir.clone(),
        }
        .into());
    }
    // A bullet under `### Changed` (or a Keep-a-Changelog sibling momo cannot
    // read) is Unreleased content momo is failing to version. A mint would
    // silently drop it from the Bump and under-version the Release, so refuse
    // rather than guess — before anything is written.
    changelog.reject_unread()?;
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

    // The Fragments are collected here, not deleted: `commit` deletes them last.
    let consumed = match mode {
        Mode::Release => recording::fragments(repo_root, config, package)?,
        Mode::Candidate => Vec::new(),
    };

    Ok(Some(Resolved {
        mode,
        version,
        changelog,
        changelog_path: package.changelog(repo_root, config),
        version_file: package.version_file(repo_root),
        consumed,
        package_dir,
    }))
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

/// The Release Candidate suffix template from the configuration.
fn rc_format(config: &Config) -> &str {
    config.rc_format.as_deref().unwrap_or(DEFAULT_RC_FORMAT)
}
