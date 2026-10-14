//! The recording model's file I/O.
//!
//! `momo-core` owns the pure shapes — the `Changelog` document and a `Change`'s
//! Fragment file name — and where a Package keeps them; this module reads and
//! writes those files, selecting the store from the resolved `Config`.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::cli::atomic;
use crate::domain::change::{Change, Kind};
use crate::domain::changelog::Changelog;
use crate::domain::config::{Config, Recording};
use crate::domain::package::Package;

/// The Unreleased Changes held by the store `config` selects for `package`.
pub fn unreleased(repo_root: &Path, config: &Config, package: &Package) -> Result<Vec<Change>> {
    match config.recording {
        Recording::Changelog => {
            let path = package.changelog(repo_root, config);
            match fs::read_to_string(&path) {
                Ok(text) => Ok(Changelog::parse(&text)
                    .with_context(|| format!("reading {}", path.display()))?
                    .into_unreleased()),
                Err(err) if err.kind() == ErrorKind::NotFound => Ok(Vec::new()),
                Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
            }
        }
        Recording::Fragments => read_fragments(&package.changes_dir(repo_root, config)),
    }
}

/// Record `change` in the store `config` selects for `package`, returning the
/// file touched.
///
/// With `dry_run`, nothing is written; the returned path is where the Change
/// would have landed.
pub fn record(
    repo_root: &Path,
    config: &Config,
    package: &Package,
    change: &Change,
    dry_run: bool,
) -> Result<PathBuf> {
    match config.recording {
        Recording::Changelog => {
            let path = package.changelog(repo_root, config);
            let mut changelog = match fs::read_to_string(&path) {
                Ok(text) => Changelog::parse(&text)
                    .with_context(|| format!("reading {}", path.display()))?,
                Err(err) if err.kind() == ErrorKind::NotFound => Changelog::empty(),
                Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
            };
            changelog.record(change.clone());
            if !dry_run {
                atomic::write(&path, &changelog.render())
                    .with_context(|| format!("writing {}", path.display()))?;
            }
            Ok(path)
        }
        Recording::Fragments => {
            let dir = package.changes_dir(repo_root, config);
            if dry_run {
                return Ok(unique_fragment_path(&dir, change));
            }
            fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
            create_fragment(&dir, change)
        }
    }
}

/// The body of a Fragment: the Change text and a trailing newline.
fn fragment_body(change: &Change) -> String {
    format!("{}\n", change.text)
}

/// Create `change`'s Fragment under `dir`, claiming the first free name.
///
/// `create_new` is what makes this safe under concurrency: two `momo add` runs
/// that pick the same name cannot both win — the loser sees `AlreadyExists` and
/// takes the next `-N` suffix.
fn create_fragment(dir: &Path, change: &Change) -> Result<PathBuf> {
    let body = fragment_body(change);
    for candidate in fragment_candidates(dir, change) {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(body.as_bytes())
                    .with_context(|| format!("writing {}", candidate.display()))?;
                return Ok(candidate);
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err).with_context(|| format!("creating {}", candidate.display()));
            }
        }
    }
    bail!("no free Fragment name under {}", dir.display())
}

/// The Fragment file names to try for `change`, the plain name first.
fn fragment_candidates<'a>(dir: &'a Path, change: &Change) -> impl Iterator<Item = PathBuf> + 'a {
    let base = dir.join(change.fragment_file_name());
    let stem = base
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("change")
        .to_owned();
    let extension = base
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("md")
        .to_owned();
    std::iter::once(base).chain((2u32..).map(move |n| dir.join(format!("{stem}-{n}.{extension}"))))
}

/// The path a Fragment *would* take under `dir` — the `--dry-run` preview.
///
/// Nothing is written, so the check-then-use race the real path avoids with
/// `create_new` cannot bite here.
fn unique_fragment_path(dir: &Path, change: &Change) -> PathBuf {
    fragment_candidates(dir, change)
        .find(|candidate| !candidate.exists())
        .unwrap_or_else(|| dir.join(change.fragment_file_name()))
}

/// Every Fragment `package` holds, sorted — what a Release consumes.
///
/// Only the fragment recording model has Fragment files, so in changelog mode
/// this is empty.
pub fn fragments(repo_root: &Path, config: &Config, package: &Package) -> Result<Vec<PathBuf>> {
    if config.recording != Recording::Fragments {
        return Ok(Vec::new());
    }

    let mut paths = fragment_paths(&package.changes_dir(repo_root, config))?;
    paths.sort();
    Ok(paths)
}

/// Delete every Fragment in `paths`.
pub fn consume(paths: &[PathBuf]) -> Result<()> {
    for path in paths {
        fs::remove_file(path).with_context(|| format!("removing {}", path.display()))?;
    }
    Ok(())
}

/// Read every Fragment under `dir` into a Change, ordered by text.
///
/// Fragment text is normalised exactly as `momo add` normalises it, so a
/// hand-authored multi-line Fragment cannot inject Changelog structure. A
/// Fragment that is empty once normalised records nothing and is skipped: a
/// stray blank file is a mistake, not a reason to fail the run.
fn read_fragments(dir: &Path) -> Result<Vec<Change>> {
    let mut changes = Vec::new();
    for path in fragment_paths(dir)? {
        let Some(kind) = fragment_kind(&path) else {
            continue;
        };
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        if let Ok(change) = Change::try_new(kind, &text) {
            changes.push(change);
        }
    }
    changes.sort_by(|a, b| a.text.cmp(&b.text));
    Ok(changes)
}

/// Every Fragment file under `dir`, unordered.
fn fragment_paths(dir: &Path) -> Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("reading {}", dir.display())),
    };

    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("reading {}", dir.display()))?
            .path();
        if fragment_kind(&path).is_some() {
            paths.push(path);
        }
    }
    Ok(paths)
}

/// The Kind a Fragment file name encodes, if it is a Fragment at all.
fn fragment_kind(path: &Path) -> Option<Kind> {
    if path.extension()?.to_str()? != "md" {
        return None;
    }
    let name = path.file_name()?.to_str()?;
    let (prefix, _) = name.split_once('-')?;
    prefix.parse::<Kind>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn a_taken_fragment_name_falls_through_to_the_next_suffix() {
        let dir = TempDir::new().expect("temp dir");
        let change = Change::new(Kind::Add, "Add a thing");
        let base = dir.path().join(change.fragment_file_name());
        fs::write(&base, "already here\n").expect("seed the base fragment");

        let path = create_fragment(dir.path(), &change).expect("create a fragment");

        assert_eq!(path, dir.path().join("add-add-a-thing-2.md"));
        assert_eq!(fs::read_to_string(&path).expect("read"), "Add a thing\n");
        assert_eq!(
            fs::read_to_string(&base).expect("read the base"),
            "already here\n",
            "the existing fragment was clobbered"
        );
    }

    #[test]
    fn the_dry_run_preview_takes_the_next_free_name_without_writing() {
        let dir = TempDir::new().expect("temp dir");
        let change = Change::new(Kind::Add, "Add a thing");
        fs::write(dir.path().join("add-add-a-thing.md"), "x").expect("seed the base fragment");

        let path = unique_fragment_path(dir.path(), &change);

        assert_eq!(path, dir.path().join("add-add-a-thing-2.md"));
        assert!(!path.exists(), "the preview wrote a fragment");
    }
}
