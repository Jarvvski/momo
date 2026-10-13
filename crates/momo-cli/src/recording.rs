//! The recording model's file I/O.
//!
//! `momo-core` owns the pure shapes — the `Changelog` document and a `Change`'s
//! Fragment file name; this module reads and writes the files that hold them,
//! selecting the store from the resolved `Config`.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use momo_core::change::{Change, Kind};
use momo_core::changelog::Changelog;
use momo_core::config::{Config, Recording};

/// The Unreleased Changes held by the store `config` selects.
pub fn unreleased(root: &Path, config: &Config) -> Result<Vec<Change>> {
    match config.recording {
        Recording::Changelog => {
            let path = root.join(&config.changelog);
            match fs::read_to_string(&path) {
                Ok(text) => Ok(Changelog::parse(&text).into_unreleased()),
                Err(err) if err.kind() == ErrorKind::NotFound => Ok(Vec::new()),
                Err(err) => Err(err).with_context(|| format!("reading {}", path.display())),
            }
        }
        Recording::Fragments => read_fragments(&root.join(&config.changes_dir)),
    }
}

/// Record `change` in the store `config` selects, returning the file touched.
///
/// With `dry_run`, nothing is written; the returned path is where the Change
/// would have landed.
pub fn record(root: &Path, config: &Config, change: Change, dry_run: bool) -> Result<PathBuf> {
    match config.recording {
        Recording::Changelog => {
            let path = root.join(&config.changelog);
            let mut changelog = match fs::read_to_string(&path) {
                Ok(text) => Changelog::parse(&text),
                Err(err) if err.kind() == ErrorKind::NotFound => Changelog::empty(),
                Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
            };
            changelog.record(change);
            if !dry_run {
                fs::write(&path, changelog.render())
                    .with_context(|| format!("writing {}", path.display()))?;
            }
            Ok(path)
        }
        Recording::Fragments => {
            let dir = root.join(&config.changes_dir);
            let path = unique_fragment_path(&dir, &change);
            if !dry_run {
                fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
                fs::write(&path, fragment_body(&change))
                    .with_context(|| format!("writing {}", path.display()))?;
            }
            Ok(path)
        }
    }
}

/// The body of a Fragment: the Change text and a trailing newline.
fn fragment_body(change: &Change) -> String {
    format!("{}\n", change.text)
}

/// A Fragment path under `dir` that does not collide with an existing file.
fn unique_fragment_path(dir: &Path, change: &Change) -> PathBuf {
    let path = dir.join(change.fragment_file_name());
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("change");
    let extension = path.extension().and_then(|s| s.to_str()).unwrap_or("md");
    (2u32..)
        .map(|n| dir.join(format!("{stem}-{n}.{extension}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(path)
}

/// Read every Fragment under `dir` into a Change, ordered by text.
fn read_fragments(dir: &Path) -> Result<Vec<Change>> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("reading {}", dir.display())),
    };

    let mut changes = Vec::new();
    for entry in entries {
        let path = entry
            .with_context(|| format!("reading {}", dir.display()))?
            .path();
        let Some(kind) = fragment_kind(&path) else {
            continue;
        };
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        changes.push(Change::new(kind, text.trim()));
    }
    changes.sort_by(|a, b| a.text.cmp(&b.text));
    Ok(changes)
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
