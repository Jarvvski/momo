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
use crate::domain::changelog::UnreadSection;
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

/// The text the global flags select: `--version-only` wins, then the format
/// (`--format`, or its `--json` shorthand).
fn render(cli: &Cli, report: &impl Report) -> String {
    if cli.version_only {
        return format!("{}\n", report.version());
    }
    match cli.output_format() {
        Format::Human => report.human(),
        Format::Json => report.json(),
    }
}

/// One dependent edge a Release would leave with an unsatisfied requirement.
///
/// `satisfied` is `false` for a requirement momo read and found unsatisfied at
/// `target`, and `null` for one it could not check: a dependent whose provider
/// has no manifest adapter, a requirement whose syntax momo does not recognise,
/// or a manifest it could not read. It is never `true` — a satisfied edge is
/// not reported, so an absent entry is the satisfied case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Cascade {
    /// The dependent Package, e.g. `cargo:b`.
    pub dependent: String,
    /// The Package being released, e.g. `cargo:a`.
    pub dependency: String,
    /// The dependent's declared requirement, when momo could read one.
    pub requirement: Option<String>,
    /// The Version the released Package would mint.
    pub target: Version,
    /// Whether the requirement is satisfied at `target`; `null` when unchecked.
    pub satisfied: Option<bool>,
}

impl Cascade {
    /// The entry's human line, without indentation.
    fn line(&self) -> String {
        match (&self.requirement, self.satisfied) {
            (Some(requirement), Some(false)) => format!(
                "{} requires {} {requirement} — unsatisfied at {}",
                self.dependent, self.dependency, self.target
            ),
            (Some(requirement), _) => format!(
                "{} requires {} {requirement} — not checked",
                self.dependent, self.dependency
            ),
            (None, _) => format!(
                "{} requires {} — {}",
                self.dependent,
                self.dependency,
                self.unchecked_reason()
            ),
        }
    }

    /// Why an entry with no requirement is unchecked.
    ///
    /// A dependent momo has no manifest adapter for cannot be read at all; one
    /// it can read but that declares no requirement for the edge is simply not
    /// something momo checked.
    fn unchecked_reason(&self) -> &'static str {
        match self.dependent.split_once(':').map(|(provider, _)| provider) {
            Some("cargo" | "node" | "uv" | "go") | None => "not checked",
            Some(_) => "unsupported ecosystem, not checked",
        }
    }
}

/// One dependent's requirement a Release widened to admit a new Version.
///
/// The counterpart of [`Cascade`] on the rewriting path: a `cascade` entry is a
/// range momo *left* unsatisfied, a `range` entry is one it rewrote. `requirement`
/// is what the dependent declared, and `target` is the Version it was widened
/// for — the requirement the run wrote is that pair resolved by the ecosystem's
/// own rules.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Range {
    /// The dependent Package whose manifest was rewritten, e.g. `cargo:b`.
    pub dependent: String,
    /// The Package being released, e.g. `cargo:a`.
    pub dependency: String,
    /// The requirement the dependent declared before the rewrite.
    pub requirement: String,
    /// The Version the requirement was widened to admit.
    pub target: Version,
}

impl Range {
    /// The entry's human line, without indentation.
    fn line(&self) -> String {
        format!(
            "{} requires {} {} — rewritten for {}",
            self.dependent, self.dependency, self.requirement, self.target
        )
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
    /// The Unreleased sections under a Keep-a-Changelog heading momo cannot
    /// read. Empty in the common case; non-empty means a mint would refuse.
    pub unversioned: Vec<UnreadSection>,
    /// The dependent edges this Package's Release would leave unsatisfied, or
    /// could not check. Empty when it has no dependents or every one is
    /// satisfied.
    pub cascade: Vec<Cascade>,
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
    /// `null`), the `next` Version a Release would mint, the `changes` that
    /// justify it, the `unversioned` sections a mint would refuse, and the
    /// `cascade` edges the Release would leave unsatisfied. `next` is computed
    /// rather than the stored field, so the envelope cannot be a derived impl.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("StatusPackage", 6)?;
        state.serialize_field("name", &self.package)?;
        state.serialize_field("current", &self.current)?;
        state.serialize_field("next", &self.computed())?;
        state.serialize_field("changes", &Changes(&self.changes))?;
        state.serialize_field("unversioned", &Unversioned(&self.unversioned))?;
        state.serialize_field("cascade", &self.cascade)?;
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
///       "changes": [{ "kind": "add", "text": "Add a shiny thing" }],
///       "unversioned": [],
///       "cascade": []
///     }
///   ]
/// }
/// ```
///
/// `unreleased` is the CI skip gate — `true` when at least one Change is
/// Unreleased. `current` is the Package's last Release (or cut Candidate), or
/// `null` when it has never released; `next` is the Version a Release would
/// mint, or the current Version when nothing is Unreleased; `changes` is empty
/// in that case. `unversioned` lists the Unreleased sections under a
/// Keep-a-Changelog heading momo cannot read — the condition that makes a mint
/// refuse; it is empty in the common case. `cascade` names the dependent edges
/// that Release would leave unsatisfied — or could not check — as
/// `{ "dependent", "dependency", "requirement", "target", "satisfied" }`.
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
            if !package.unversioned.is_empty() {
                out.push_str("    Unversioned\n");
                for unread in &package.unversioned {
                    let noun = if unread.entries == 1 {
                        "entry"
                    } else {
                        "entries"
                    };
                    out.push_str(&format!(
                        "      - `### {}` (line {}): {} {noun} momo cannot version\n",
                        unread.heading, unread.line, unread.entries,
                    ));
                }
            }
            let current = match &package.current {
                Some(version) => version.to_string(),
                None => "—".to_owned(),
            };
            out.push_str(&format!("    current: {current}\n"));
            out.push_str(&format!("    → next: {}\n", package.computed()));
            for cascade in &package.cascade {
                out.push_str(&format!("    cascade: {}\n", cascade.line()));
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
    /// The dependent edges this Package's Release would leave unsatisfied, or
    /// could not check. Empty when it has no dependents or every one is
    /// satisfied.
    pub cascade: Vec<Cascade>,
    /// The dependent requirements this Release widened in the dependents it is
    /// releasing alongside it.
    ///
    /// Empty unless the run opted in *and* a dependent in the same release set
    /// declared a range the new Version does not satisfy. A dependent outside
    /// the set is never rewritten, so it appears in `cascade` instead.
    pub ranges: Vec<Range>,
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
///       "consumed": [".momo/changes/add-a-thing.md"],
///       "cascade": [],
///       "ranges": []
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
/// never silent. `cascade` names the dependent edges the new Version leaves
/// unsatisfied, or could not check — `[]` when there are none. `ranges` names
/// the dependent requirements the run *rewrote*, as
/// `{ "dependent", "dependency", "requirement", "target" }` — `[]` unless
/// `release --sync-ranges` opted in. Under `--dry-run` the paths are what the
/// run *would* have written.
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
            for cascade in &package.cascade {
                out.push_str(&format!("cascade: {}\n", cascade.line()));
            }
            for range in &package.ranges {
                out.push_str(&format!("range: {}\n", range.line()));
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

/// One Unreleased section under a heading momo cannot read, as `status` reports
/// it: its heading, its line, and how many entries it holds.
#[derive(Serialize)]
struct UnreadSectionReport<'a> {
    /// The heading, e.g. `Changed`, without the `### ` prefix.
    heading: &'a str,
    /// The 1-based document line of the heading.
    line: usize,
    /// The number of `- ` bullets beneath it.
    entries: usize,
}

/// The `unversioned` array of a `status` report, rendered from the domain's
/// [`UnreadSection`]s.
struct Unversioned<'a>(&'a [UnreadSection]);

impl Serialize for Unversioned<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for section in self.0 {
            seq.serialize_element(&UnreadSectionReport {
                heading: &section.heading,
                line: section.line,
                entries: section.entries,
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
