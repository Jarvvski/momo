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
    /// The ecosystem manifest the run synced, when it found one.
    #[serde(serialize_with = "serialize_optional_path")]
    pub manifest: Option<PathBuf>,
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
///       "manifest": "Cargo.toml",
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
/// the ecosystem manifest the run synced, or `null` when the Package has none.
/// Under `--dry-run` the paths are what the run *would* have written.
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
            if let Some(manifest) = &package.manifest {
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

/// One Package's entry in a `notes` report.
#[derive(Serialize)]
pub struct NotesPackage {
    /// The Package's identity, e.g. `cargo:momo`, reported as `name`.
    #[serde(rename = "name")]
    pub package: String,
    /// The Version the section describes.
    pub version: Version,
    /// The section body, verbatim.
    pub notes: String,
}

/// What `momo notes` reports: the Changelog section for a released Version.
///
/// The JSON shape is:
///
/// ```json
/// {
///   "packages": [
///     {
///       "name": "momo",
///       "version": "1.4.2",
///       "notes": "### Added\n\n- Add a shiny thing"
///     }
///   ]
/// }
/// ```
///
/// Like the other reporting verbs, `packages` is always an array, so a
/// monorepo's run is one document and one `--output` file. There is no
/// `dry_run`: `notes` reads only, so there is nothing for it to withhold.
#[derive(Serialize)]
pub struct Notes {
    /// The Packages, in discovery order.
    pub packages: Vec<NotesPackage>,
}

impl Report for Notes {
    /// A single Package's body alone: notes are published, not read, so a lone
    /// Package needs no header. With several, each is introduced by its Package
    /// and Version so the bodies can be told apart.
    fn human(&self) -> String {
        if let [package] = self.packages.as_slice() {
            return format!("{}\n", package.notes);
        }
        let mut out = String::new();
        for (index, package) in self.packages.iter().enumerate() {
            if index > 0 {
                out.push('\n');
            }
            out.push_str(&format!("## {} {}\n\n", package.package, package.version));
            out.push_str(&format!("{}\n", package.notes));
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

/// [`serialize_path`] for an optional path: `null` when the Package has none.
fn serialize_optional_path<S>(path: &Option<PathBuf>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match path {
        Some(path) => serialize_path(path, serializer),
        None => serializer.serialize_none(),
    }
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
mod tests {
    use super::*;

    use std::path::PathBuf;

    /// A `Cli` with every global flag at its default.
    fn cli() -> Cli {
        Cli {
            command: crate::cli::Command::Status { check: false },
            dry_run: false,
            format: None,
            version_only: false,
            output: None,
            package: None,
            all: false,
        }
    }

    /// A status report for one never-released Package holding `changes`.
    fn status(changes: &[Change]) -> Status {
        let next = crate::domain::version::next_version(changes, &Version::new(0, 0, 0))
            .expect("a bump that fits");
        Status {
            packages: vec![StatusPackage {
                package: "momo".to_owned(),
                current: None,
                next,
                changes: changes.to_vec(),
            }],
        }
    }

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    #[test]
    fn status_json_has_the_documented_shape() {
        let changes = [Change::new(Kind::Add, "Add a shiny thing")];
        let json = status(&changes).json();

        assert_eq!(
            json,
            r#"{
  "unreleased": true,
  "packages": [
    {
      "name": "momo",
      "current": null,
      "next": "0.1.0",
      "changes": [
        {
          "kind": "add",
          "text": "Add a shiny thing"
        }
      ]
    }
  ]
}
"#
        );
    }

    #[test]
    fn status_json_reports_nothing_unreleased_as_the_current_version() {
        let json = status(&[]).json();

        assert!(json.contains("\"unreleased\": false"), "{json}");
        assert!(json.contains("\"next\": \"0.0.0\""), "{json}");
        assert!(json.contains("\"current\": null"), "{json}");
        assert!(json.contains("\"changes\": []"), "{json}");
    }

    #[test]
    fn status_json_distinguishes_a_zero_release_from_never_released() {
        // A Package that has genuinely released `0.0.0` reports it; one that has
        // never released reports `null`. The two are not the same.
        let released = Status {
            packages: vec![StatusPackage {
                package: "momo".to_owned(),
                current: Some(Version::new(0, 0, 0)),
                next: None,
                changes: Vec::new(),
            }],
        };
        assert!(
            released.json().contains("\"current\": \"0.0.0\""),
            "{}",
            released.json()
        );
        assert!(status(&[]).json().contains("\"current\": null"));
    }

    #[test]
    fn status_json_keeps_changes_in_kind_then_text_order() {
        let changes = [
            Change::new(Kind::Break, "Break it"),
            Change::new(Kind::Add, "Add it"),
            Change::new(Kind::Fix, "Fix it"),
        ];
        let json = status(&changes).json();

        let positions: Vec<usize> = ["break", "add", "fix"]
            .iter()
            .map(|kind| {
                json.find(&format!("\"kind\": \"{kind}\""))
                    .unwrap_or_else(|| panic!("`{kind}` missing:\n{json}"))
            })
            .collect();
        assert!(
            positions[0] < positions[1] && positions[1] < positions[2],
            "{json}"
        );
    }

    #[test]
    fn status_json_escapes_change_text() {
        let changes = [Change::new(Kind::Add, "A \"quoted\"\tchange")];
        let json = status(&changes).json();

        assert!(json.contains(r#""text": "A \"quoted\"\tchange""#), "{json}");
    }

    #[test]
    fn status_json_escapes_a_control_character_as_an_escape_sequence() {
        // A control character below `0x20` has no short escape, so it must come
        // out as `\u00XX` for the document to stay one line — and parse back
        // verbatim.
        let changes = [Change::new(Kind::Fix, "a\u{1}b")];
        let json = status(&changes).json();

        assert!(json.contains(r#""text": "a\u0001b""#), "{json}");
        let parsed: serde_json::Value =
            serde_json::from_str(&json).expect("the report is one JSON document");
        assert_eq!(parsed["packages"][0]["changes"][0]["text"], "a\u{1}b");
    }

    #[test]
    fn status_human_lists_kinds_and_ends_with_the_next_version() {
        let changes = [
            Change::new(Kind::Add, "Add a shiny thing"),
            Change::new(Kind::Fix, "Fix a nasty bug"),
        ];
        let human = status(&changes).human();

        assert_eq!(
            human,
            "Unreleased\n\n  momo\n    Added\n      - Add a shiny thing\n    Fixed\n      - Fix a nasty bug\n    → next: 0.1.0\n"
        );
    }

    #[test]
    fn status_human_with_nothing_unreleased_reports_the_current_version() {
        let human = status(&[]).human();

        assert_eq!(human, "Unreleased\n\n  momo\n    → next: 0.0.0\n");
    }

    #[test]
    fn status_version_only_is_the_bare_computed_version() {
        let changes = [Change::new(Kind::Fix, "Fix a nasty bug")];

        assert_eq!(status(&changes).version(), "0.0.1");
        assert_eq!(status(&[]).version(), "0.0.0");
    }

    #[test]
    fn status_check_gate_tracks_the_unreleased_changes() {
        assert!(status(&[Change::new(Kind::Add, "Add it")]).unreleased());
        assert!(!status(&[]).unreleased());
    }

    #[test]
    fn status_reports_every_package() {
        let report = Status {
            packages: vec![
                StatusPackage {
                    package: "cargo:a".to_owned(),
                    current: None,
                    next: Some(Version::new(0, 1, 0)),
                    changes: vec![Change::new(Kind::Add, "Add a thing")],
                },
                StatusPackage {
                    package: "cargo:b".to_owned(),
                    current: None,
                    next: None,
                    changes: Vec::new(),
                },
            ],
        };

        let human = report.human();
        assert!(human.contains("  cargo:a"), "{human}");
        assert!(human.contains("  cargo:b"), "{human}");
        assert!(human.contains("→ next: 0.1.0"), "{human}");
        assert!(human.contains("→ next: 0.0.0"), "{human}");
        assert!(report.unreleased());
        assert_eq!(report.version(), "0.1.0\n0.0.0");

        let json = report.json();
        assert!(json.contains("\"unreleased\": true"), "{json}");
        assert!(json.contains("\"name\": \"cargo:a\""), "{json}");
        assert!(json.contains("\"name\": \"cargo:b\""), "{json}");
    }

    /// One Package's entry in a `release` envelope, with the fixture paths.
    fn package_entry(name: &str, text: &str) -> ReleasePackage {
        ReleasePackage {
            package: name.to_owned(),
            version: version(text),
            changelog: PathBuf::from("/repo/CHANGELOG.md"),
            version_file: PathBuf::from("/repo/.momo/version"),
            manifest: Some(PathBuf::from("/repo/Cargo.toml")),
            consumed: vec![PathBuf::from("/repo/.momo/changes/add-a-thing.md")],
        }
    }

    /// One Package's entry in a `tag` envelope.
    fn tag_entry(package: &str, name: &str, text: &str) -> TagPackage {
        TagPackage {
            package: package.to_owned(),
            name: name.to_owned(),
            version: version(text),
        }
    }

    #[test]
    fn release_json_has_the_documented_shape() {
        let report = Release {
            verb: Verb::Release,
            dry_run: false,
            packages: vec![package_entry("momo", "0.1.0")],
        };

        assert_eq!(
            report.json(),
            r#"{
  "dry_run": false,
  "packages": [
    {
      "name": "momo",
      "version": "0.1.0",
      "changelog": "/repo/CHANGELOG.md",
      "version_file": "/repo/.momo/version",
      "manifest": "/repo/Cargo.toml",
      "consumed": [
        "/repo/.momo/changes/add-a-thing.md"
      ]
    }
  ]
}
"#
        );
    }

    #[test]
    fn release_json_reports_a_missing_manifest_as_null() {
        let report = Release {
            verb: Verb::Release,
            dry_run: false,
            packages: vec![ReleasePackage {
                manifest: None,
                ..package_entry("momo", "0.1.0")
            }],
        };

        assert!(
            report.json().contains("\"manifest\": null"),
            "{}",
            report.json()
        );
    }

    #[test]
    fn release_json_reports_every_package_in_one_document() {
        let report = Release {
            verb: Verb::Release,
            dry_run: false,
            packages: vec![
                package_entry("cargo:a", "0.1.0"),
                package_entry("cargo:b", "0.2.0"),
            ],
        };

        let json = report.json();
        assert!(json.contains("\"name\": \"cargo:a\""), "{json}");
        assert!(json.contains("\"name\": \"cargo:b\""), "{json}");
        assert_eq!(json.matches("\"packages\":").count(), 1, "{json}");
    }

    #[test]
    fn release_human_names_the_verb_and_what_it_wrote() {
        let report = Release {
            verb: Verb::Release,
            dry_run: false,
            packages: vec![package_entry("momo", "0.1.0")],
        };

        assert_eq!(
            report.human(),
            "released 0.1.0\nwrote /repo/CHANGELOG.md\nwrote /repo/.momo/version\nwrote /repo/Cargo.toml\nremoved /repo/.momo/changes/add-a-thing.md\n"
        );
    }

    #[test]
    fn release_human_speaks_in_the_conditional_under_dry_run() {
        let report = Release {
            verb: Verb::Candidate,
            dry_run: true,
            packages: vec![ReleasePackage {
                version: version("0.1.0-rc.1"),
                manifest: None,
                consumed: Vec::new(),
                ..package_entry("momo", "0.0.0")
            }],
        };

        assert_eq!(
            report.human(),
            "would cut 0.1.0-rc.1\nwould write /repo/CHANGELOG.md\nwould write /repo/.momo/version\n"
        );
    }

    #[test]
    fn release_version_only_is_the_minted_version() {
        let report = Release {
            verb: Verb::Release,
            dry_run: false,
            packages: vec![package_entry("momo", "1.2.3")],
        };

        assert_eq!(report.version(), "1.2.3");
    }

    #[test]
    fn release_version_only_joins_every_package() {
        let report = Release {
            verb: Verb::Release,
            dry_run: false,
            packages: vec![
                package_entry("cargo:a", "1.2.3"),
                package_entry("cargo:b", "4.5.6"),
            ],
        };

        assert_eq!(report.version(), "1.2.3\n4.5.6");
    }

    #[test]
    fn tag_json_has_the_documented_shape() {
        let report = Tag {
            dry_run: false,
            packages: vec![tag_entry("momo", "v0.1.0", "0.1.0")],
        };

        assert_eq!(
            report.json(),
            r#"{
  "dry_run": false,
  "packages": [
    {
      "name": "momo",
      "tag": "v0.1.0",
      "version": "0.1.0"
    }
  ]
}
"#
        );
    }

    #[test]
    fn tag_json_reports_every_package_in_one_document() {
        let report = Tag {
            dry_run: false,
            packages: vec![
                tag_entry("cargo:a", "cargo-a-v0.1.0", "0.1.0"),
                tag_entry("cargo:b", "cargo-b-v0.2.0", "0.2.0"),
            ],
        };

        let json = report.json();
        assert!(json.contains("\"tag\": \"cargo-a-v0.1.0\""), "{json}");
        assert!(json.contains("\"tag\": \"cargo-b-v0.2.0\""), "{json}");
        assert_eq!(json.matches("\"packages\":").count(), 1, "{json}");
    }

    #[test]
    fn tag_human_speaks_in_the_conditional_under_dry_run() {
        assert_eq!(
            Tag {
                dry_run: true,
                packages: vec![tag_entry("momo", "v0.1.0", "0.1.0")],
            }
            .human(),
            "would tag v0.1.0\n"
        );
    }

    /// One Package's entry in a `notes` report.
    fn notes_entry(package: &str, text: &str, notes: &str) -> NotesPackage {
        NotesPackage {
            package: package.to_owned(),
            version: version(text),
            notes: notes.to_owned(),
        }
    }

    #[test]
    fn notes_json_has_the_documented_shape() {
        let report = Notes {
            packages: vec![notes_entry(
                "momo",
                "1.4.2",
                "- Add a shiny thing\n- Fix a nasty bug",
            )],
        };

        assert_eq!(
            report.json(),
            r#"{
  "packages": [
    {
      "name": "momo",
      "version": "1.4.2",
      "notes": "- Add a shiny thing\n- Fix a nasty bug"
    }
  ]
}
"#
        );
    }

    #[test]
    fn notes_json_reports_every_package_in_one_document() {
        let report = Notes {
            packages: vec![
                notes_entry("cargo:a", "0.1.0", "- a thing"),
                notes_entry("cargo:b", "0.2.0", "- another thing"),
            ],
        };

        let json = report.json();
        assert!(json.contains("\"name\": \"cargo:a\""), "{json}");
        assert!(json.contains("\"name\": \"cargo:b\""), "{json}");
        assert_eq!(json.matches("\"packages\":").count(), 1, "{json}");
    }

    #[test]
    fn notes_human_is_the_body_alone_for_one_package() {
        // The notes are published rather than read, so a lone Package adds no
        // header to the release body.
        let report = Notes {
            packages: vec![notes_entry("momo", "1.4.2", "### Added\n\n- Add a thing")],
        };

        assert_eq!(report.human(), "### Added\n\n- Add a thing\n");
    }

    #[test]
    fn notes_human_names_each_package_when_there_are_several() {
        let report = Notes {
            packages: vec![
                notes_entry("cargo:a", "0.1.0", "- a thing"),
                notes_entry("cargo:b", "0.2.0", "- another thing"),
            ],
        };

        assert_eq!(
            report.human(),
            "## cargo:a 0.1.0\n\n- a thing\n\n## cargo:b 0.2.0\n\n- another thing\n"
        );
    }

    #[test]
    fn notes_version_only_is_the_described_version() {
        let report = Notes {
            packages: vec![notes_entry("momo", "1.4.2", "- a thing")],
        };

        assert_eq!(report.version(), "1.4.2");
    }

    #[test]
    fn version_only_wins_over_the_format() {
        let mut cli = cli();
        cli.version_only = true;
        cli.format = Some(Format::Json);

        assert_eq!(render(&cli, &status(&[])), "0.0.0\n");
    }

    #[test]
    fn the_format_selects_the_rendering() {
        let mut cli = cli();
        cli.format = Some(Format::Json);

        assert!(render(&cli, &status(&[])).starts_with("{\n"));
    }

    #[test]
    fn emit_writes_to_the_output_path_instead_of_stdout() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("status.json");
        let mut cli = cli();
        cli.format = Some(Format::Json);
        cli.output = Some(path.clone());

        emit(&cli, &status(&[])).expect("emit");

        let written = fs::read_to_string(&path).expect("read the written file");
        assert_eq!(written, status(&[]).json());
    }
}
