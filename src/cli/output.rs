//! Rendering command output for humans, machines, and CI.
//!
//! A reporting command builds a [`Report`] and hands it to [`emit`], which picks
//! the rendering the global flags ask for (`--format`, `--version-only`) and
//! sends it to stdout or, with `--output <path>`, to a file. The JSON shapes are
//! a deliberate contract: the report types derive [`Serialize`], and the shapes
//! are pinned by tests.

use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::change::{Change, Kind};
use anyhow::{Context, Result};
use semver::Version;
use serde::Serialize;
use serde::ser::{SerializeSeq, SerializeStruct, Serializer};

use crate::cli::{Cli, Format};

/// Something a reporting command emits.
pub trait Report {
    /// The human-readable rendering, newline-terminated.
    fn human(&self) -> String;

    /// The stable JSON rendering, newline-terminated.
    fn json(&self) -> String;

    /// The bare Version `--version-only` prints.
    fn version(&self) -> String;
}

/// Render `report` as the global flags ask, to stdout or `--output <path>`.
pub fn emit(cli: &Cli, report: &impl Report) -> Result<()> {
    let text = render(cli, report);
    match &cli.output {
        Some(path) => fs::write(path, text).with_context(|| format!("writing {}", path.display())),
        None => {
            print!("{text}");
            Ok(())
        }
    }
}

/// The text the global flags select: `--version-only` wins, then `--format`.
fn render(cli: &Cli, report: &impl Report) -> String {
    if cli.version_only {
        return format!("{}\n", report.version());
    }
    match cli.format.unwrap_or_default() {
        Format::Human => report.human(),
        Format::Json => report.json(),
    }
}

/// One Package's line in a `status` report.
pub struct StatusPackage {
    /// The Package's identity, e.g. `cargo:momo`.
    pub package: String,
    /// The Package's current Version — the last Release or cut Candidate —
    /// or `None` when it has never released.
    pub current: Option<Version>,
    /// The Version a Release would mint; `None` when nothing is Unreleased.
    pub next: Option<Version>,
    /// The Unreleased Changes.
    pub changes: Vec<Change>,
}

impl StatusPackage {
    /// Whether this Package has anything Unreleased to release.
    fn unreleased(&self) -> bool {
        !self.changes.is_empty()
    }

    /// The Version to report: the one a Release would mint, the current Version,
    /// or the bootstrap baseline when the Package has never released.
    fn computed(&self) -> Version {
        self.next
            .clone()
            .or_else(|| self.current.clone())
            .unwrap_or_else(|| Version::new(0, 0, 0))
    }
}

impl Serialize for StatusPackage {
    /// The documented shape: the Package's `name`, its `current` Version (or
    /// `null`), the `next` Version a Release would mint, and the `changes` that
    /// justify it. `next` is computed rather than the stored field, so the
    /// envelope cannot be a derived impl.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("StatusPackage", 4)?;
        state.serialize_field("name", &self.package)?;
        state.serialize_field("current", &self.current)?;
        state.serialize_field("next", &self.computed())?;
        state.serialize_field("changes", &Changes(&self.changes))?;
        state.end()
    }
}

/// What `momo status` reports: the Unreleased Changes and the Version they
/// imply, per Package.
///
/// The JSON shape is:
///
/// ```json
/// {
///   "unreleased": true,
///   "packages": [
///     {
///       "name": "momo",
///       "current": null,
///       "next": "0.1.0",
///       "changes": [{ "kind": "add", "text": "Add a shiny thing" }]
///     }
///   ]
/// }
/// ```
///
/// `unreleased` is the CI skip gate — `true` when at least one Change is
/// Unreleased. `current` is the Package's last Release (or cut Candidate), or
/// `null` when it has never released; `next` is the Version a Release would
/// mint, or the current Version when nothing is Unreleased; `changes` is empty
/// in that case.
pub struct Status {
    /// The Packages, in discovery order.
    pub packages: Vec<StatusPackage>,
}

impl Status {
    /// Whether any Package has anything Unreleased — the `--check` gate.
    pub fn unreleased(&self) -> bool {
        self.packages.iter().any(StatusPackage::unreleased)
    }
}

impl Serialize for Status {
    /// The documented shape: the run-wide `unreleased` gate, then the packages.
    /// The gate is computed, so the envelope cannot be a derived impl.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Status", 2)?;
        state.serialize_field("unreleased", &self.unreleased())?;
        state.serialize_field("packages", &self.packages)?;
        state.end()
    }
}

impl Report for Status {
    fn human(&self) -> String {
        let mut out = String::from("Unreleased\n\n");
        for (index, package) in self.packages.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(&format!("  {}\n", package.package));
            for kind in Kind::ALL {
                let mut matching = package
                    .changes
                    .iter()
                    .filter(|change| change.kind == kind)
                    .peekable();
                if matching.peek().is_none() {
                    continue;
                }
                out.push_str(&format!("    {}\n", kind.heading()));
                for change in matching {
                    out.push_str(&format!("      - {}\n", change.text));
                }
            }
            let current = match &package.current {
                Some(version) => version.to_string(),
                None => "—".to_owned(),
            };
            out.push_str(&format!("    current: {current}\n"));
            out.push_str(&format!("    → next: {}\n", package.computed()));
        }
        out
    }

    fn json(&self) -> String {
        to_document(self)
    }

    fn version(&self) -> String {
        self.packages
            .iter()
            .map(|package| package.computed().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// The verb a minting run is reported under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// `momo release`.
    Release,
    /// `momo candidate`.
    Candidate,
}

impl Verb {
    /// The infinitive — how a `--dry-run` names the run.
    fn infinitive(self) -> &'static str {
        match self {
            Verb::Release => "release",
            Verb::Candidate => "cut",
        }
    }

    /// The past tense — how a run that happened is named.
    fn past(self) -> &'static str {
        match self {
            Verb::Release => "released",
            Verb::Candidate => "cut",
        }
    }
}

/// One manifest a `release` / `candidate` run considered but did not write.
#[derive(Serialize)]
pub struct SkippedManifest {
    /// The manifest that was left alone.
    #[serde(serialize_with = "serialize_path")]
    pub path: PathBuf,
    /// Why, e.g. `no version momo owns` or `already current`.
    pub reason: String,
}

/// One Package's entry in a `release` / `candidate` report.
#[derive(Serialize)]
pub struct ReleasePackage {
    /// The Package's identity, e.g. `cargo:momo`, reported as `name`.
    #[serde(rename = "name")]
    pub package: String,
    /// The Version minted.
    pub version: Version,
    /// The Changelog the run wrote.
    #[serde(serialize_with = "serialize_path")]
    pub changelog: PathBuf,
    /// The version file the run wrote. Both `release` and `candidate` advance
    /// it, so it is always present and the run can be tagged.
    #[serde(serialize_with = "serialize_path")]
    pub version_file: PathBuf,
    /// The ecosystem manifests the run synced, in the order it wrote them.
    ///
    /// A plan may write more than one, and an inherited version writes the
    /// shared workspace root rather than the Package's own manifest.
    #[serde(serialize_with = "serialize_paths")]
    pub manifest: Vec<PathBuf>,
    /// The manifests the run considered but did not write, each with why.
    pub skipped: Vec<SkippedManifest>,
    /// The Fragments the run consumed.
    #[serde(serialize_with = "serialize_paths")]
    pub consumed: Vec<PathBuf>,
}

/// What `momo release` / `momo candidate` report.
///
/// The JSON shape is:
///
/// ```json
/// {
///   "dry_run": false,
///   "packages": [
///     {
///       "name": "momo",
///       "version": "0.1.0",
///       "changelog": "CHANGELOG.md",
///       "version_file": ".momo/version",
///       "manifest": ["Cargo.toml"],
///       "skipped": [],
///       "consumed": [".momo/changes/add-a-thing.md"]
///     }
///   ]
/// }
/// ```
///
/// The envelope mirrors [`Status`]: `packages` is always an array, even for a
/// single-package repo, so a monorepo's run is one document and one `--output`
/// file. `dry_run` is the run-wide flag. `version_file` is always present — both
/// `release` and `candidate` advance it, so the run can be tagged. `manifest` is
/// the ecosystem manifests the run synced — `[]` when the Package has none —
/// and `skipped` names the manifests it considered but left alone, so a skip is
/// never silent. Under `--dry-run` the paths are what the run *would* have
/// written.
#[derive(Serialize)]
pub struct Release {
    /// Which verb minted the Version. The two verbs share one document, so it
    /// has no JSON rendering of its own.
    #[serde(skip)]
    pub verb: Verb,
    /// Whether nothing was actually written.
    pub dry_run: bool,
    /// The Packages, in discovery order.
    pub packages: Vec<ReleasePackage>,
}

impl Report for Release {
    fn human(&self) -> String {
        let verb = if self.dry_run {
            format!("would {}", self.verb.infinitive())
        } else {
            self.verb.past().to_owned()
        };
        let wrote = if self.dry_run { "would write" } else { "wrote" };
        let removed = if self.dry_run {
            "would remove"
        } else {
            "removed"
        };

        let mut out = String::new();
        for package in &self.packages {
            out.push_str(&format!("{verb} {}\n", package.version));
            out.push_str(&format!("{wrote} {}\n", package.changelog.display()));
            out.push_str(&format!("{wrote} {}\n", package.version_file.display()));
            for manifest in &package.manifest {
                out.push_str(&format!("{wrote} {}\n", manifest.display()));
            }
            for path in &package.consumed {
                out.push_str(&format!("{removed} {}\n", path.display()));
            }
        }
        out
    }

    fn json(&self) -> String {
        to_document(self)
    }

    fn version(&self) -> String {
        self.packages
            .iter()
            .map(|package| package.version.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// One Package's entry in a `tag` report.
#[derive(Serialize)]
pub struct TagPackage {
    /// The Package's identity, e.g. `cargo:momo`, reported as `name`.
    #[serde(rename = "name")]
    pub package: String,
    /// The tag name, prefix included, reported as `tag`.
    #[serde(rename = "tag")]
    pub name: String,
    /// The Version the tag names.
    pub version: Version,
}

/// What `momo tag` reports.
///
/// The JSON shape is:
///
/// ```json
/// {
///   "dry_run": false,
///   "packages": [
///     { "name": "momo", "tag": "v0.1.0", "version": "0.1.0" }
///   ]
/// }
/// ```
///
/// Like [`Status`] and [`Release`], `packages` is always an array and `dry_run`
/// is the run-wide flag.
#[derive(Serialize)]
pub struct Tag {
    /// Whether nothing was actually created.
    pub dry_run: bool,
    /// The Packages, in discovery order.
    pub packages: Vec<TagPackage>,
}

impl Report for Tag {
    fn human(&self) -> String {
        let verb = if self.dry_run { "would tag" } else { "tagged" };
        let mut out = String::new();
        for package in &self.packages {
            out.push_str(&format!("{verb} {}\n", package.name));
        }
        out
    }

    fn json(&self) -> String {
        to_document(self)
    }

    fn version(&self) -> String {
        self.packages
            .iter()
            .map(|package| package.version.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

/// One Unreleased Change as `status` reports it: its Kind and its text.
#[derive(Serialize)]
struct ChangeReport<'a> {
    /// The Change's Kind, e.g. `add`.
    kind: String,
    /// The Change's text, verbatim.
    text: &'a str,
}

/// The `changes` array of a `status` report, rendered from the domain's
/// [`Change`]s — which carry no serialization of their own.
struct Changes<'a>(&'a [Change]);

impl Serialize for Changes<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for change in self.0 {
            seq.serialize_element(&ChangeReport {
                kind: change.kind.to_string(),
                text: &change.text,
            })?;
        }
        seq.end()
    }
}

/// Render `report` as a pretty JSON document, newline-terminated.
///
/// The reports hold only owned data, so serialization cannot fail.
fn to_document(report: &impl Serialize) -> String {
    let mut json = serde_json::to_string_pretty(report)
        .expect("a report holds only owned data, so it always serializes");
    json.push('\n');
    json
}

/// Serialize a filesystem path the way the reports always have: through
/// [`Path::display`], which renders a non-UTF-8 path lossily rather than failing
/// the whole document.
fn serialize_path<S>(path: &Path, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.collect_str(&path.display())
}

/// [`serialize_path`] for a list of paths.
fn serialize_paths<S>(paths: &[PathBuf], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .serialize(serializer)
}

#[cfg(test)]
mod tests;
