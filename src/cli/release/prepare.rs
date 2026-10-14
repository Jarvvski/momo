//! The prepare path and the staged write transaction.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use time::Date;

use crate::cli::atomic;
use crate::cli::recording;
use crate::cli::version::{release_base, source};
use crate::domain::changelog::Links;
use crate::domain::config::{Config, Recording};
use crate::domain::package::Package;
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

/// Resolve a Release for `package` without writing anything.
///
/// The read-only half of a Release: the Changelog is read, the Version minted,
/// and every artefact — the manifest included — rendered. Because nothing is
/// written, a caller can resolve every Package in a run and validate the whole
/// set before the first byte lands. `None` when there is nothing Unreleased.
pub fn prepare_release(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
) -> Result<Option<Prepared>> {
    prepare(repo_root, config, package, vcs, date, Mode::Release)
}

/// Resolve a Release Candidate for `package` without writing anything.
pub fn prepare_candidate(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
) -> Result<Option<Prepared>> {
    prepare(repo_root, config, package, vcs, date, Mode::Candidate)
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

/// The shared body of [`prepare_release`] and [`prepare_candidate`].
fn prepare(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    vcs: Option<&dyn Vcs>,
    date: Date,
    mode: Mode,
) -> Result<Option<Prepared>> {
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

    // Rendering the manifest parses it — the step that can fail on a malformed
    // manifest — so it runs here, before a single target is touched. The
    // Fragments are collected, not deleted.
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

    Ok(Some(Prepared { outcome, writes }))
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
