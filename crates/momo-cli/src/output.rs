//! Rendering command output for humans, machines, and CI.
//!
//! A reporting command builds a [`Report`] and hands it to [`emit`], which picks
//! the rendering the global flags ask for (`--format`, `--version-only`) and
//! sends it to stdout or, with `--output <path>`, to a file. The JSON shapes are
//! a deliberate contract — the workspace carries no JSON dependency, so they are
//! assembled by hand ([`Json`]) and pinned by tests.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use momo_core::change::{Change, Kind};
use semver::Version;

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
    match cli.format {
        Format::Human => report.human(),
        Format::Json => report.json(),
    }
}

/// What `momo status` reports: the Unreleased Changes and the Version they imply.
///
/// The JSON shape is:
///
/// ```json
/// {
///   "unreleased": true,
///   "packages": [
///     {
///       "name": "momo",
///       "current": "0.0.0",
///       "next": "0.1.0",
///       "changes": [{ "kind": "add", "text": "Add a shiny thing" }]
///     }
///   ]
/// }
/// ```
///
/// `unreleased` is the CI skip gate — `true` when at least one Change is
/// Unreleased. `next` is the Version a Release would mint, or the current
/// Version when nothing is Unreleased; `changes` is empty in that case.
pub struct Status<'a> {
    /// The Package's identity, e.g. `cargo:momo`.
    pub package: &'a str,
    /// The Package's current Version.
    pub current: Version,
    /// The Version a Release would mint; `None` when nothing is Unreleased.
    pub next: Option<Version>,
    /// The Unreleased Changes.
    pub changes: &'a [Change],
}

impl Status<'_> {
    /// Whether there is anything Unreleased to release — the `--check` gate.
    pub fn unreleased(&self) -> bool {
        !self.changes.is_empty()
    }

    /// The Version to report: the one a Release would mint, or the current one.
    fn computed(&self) -> &Version {
        self.next.as_ref().unwrap_or(&self.current)
    }
}

impl Report for Status<'_> {
    fn human(&self) -> String {
        let mut out = String::from("Unreleased\n\n");
        out.push_str(&format!("  {}\n", self.package));
        for kind in Kind::ALL {
            let mut matching = self
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
        out.push_str(&format!("    → next: {}\n", self.computed()));
        out
    }

    fn json(&self) -> String {
        let changes = self
            .changes
            .iter()
            .map(|change| {
                Json::object(vec![
                    ("kind", Json::from(change.kind.to_string())),
                    ("text", Json::from(change.text.as_str())),
                ])
            })
            .collect();
        let package = Json::object(vec![
            ("name", Json::from(self.package)),
            ("current", Json::from(&self.current)),
            ("next", Json::from(self.computed())),
            ("changes", Json::Array(changes)),
        ]);
        Json::object(vec![
            ("unreleased", Json::from(self.unreleased())),
            ("packages", Json::Array(vec![package])),
        ])
        .to_document()
    }

    fn version(&self) -> String {
        self.computed().to_string()
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

/// What `momo release` / `momo candidate` report.
///
/// The JSON shape is:
///
/// ```json
/// {
///   "version": "0.1.0",
///   "changelog": "CHANGELOG.md",
///   "version_file": ".momo/version",
///   "consumed": [".momo/changes/add-a-thing.md"],
///   "dry_run": false
/// }
/// ```
///
/// `version_file` is `null` only when the run wrote no version file; under
/// `--dry-run` the paths are what the run *would* have written.
pub struct Release<'a> {
    /// The Version minted.
    pub version: &'a Version,
    /// The Changelog the run wrote.
    pub changelog: &'a Path,
    /// The version file the run wrote, when it wrote one.
    pub version_file: Option<&'a Path>,
    /// The Fragments the run consumed.
    pub consumed: &'a [PathBuf],
    /// Which verb minted the Version.
    pub verb: Verb,
    /// Whether nothing was actually written.
    pub dry_run: bool,
}

impl Report for Release<'_> {
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

        let mut out = format!("{verb} {}\n", self.version);
        out.push_str(&format!("{wrote} {}\n", self.changelog.display()));
        if let Some(path) = self.version_file {
            out.push_str(&format!("{wrote} {}\n", path.display()));
        }
        for path in self.consumed {
            out.push_str(&format!("{removed} {}\n", path.display()));
        }
        out
    }

    fn json(&self) -> String {
        let consumed = self
            .consumed
            .iter()
            .map(|path| Json::from(path.as_path()))
            .collect();
        Json::object(vec![
            ("version", Json::from(self.version)),
            ("changelog", Json::from(self.changelog)),
            (
                "version_file",
                self.version_file.map_or(Json::Null, Json::from),
            ),
            ("consumed", Json::Array(consumed)),
            ("dry_run", Json::from(self.dry_run)),
        ])
        .to_document()
    }

    fn version(&self) -> String {
        self.version.to_string()
    }
}

/// What `momo tag` reports.
///
/// The JSON shape is `{ "tag": "v0.1.0", "version": "0.1.0", "dry_run": false }`.
pub struct Tag<'a> {
    /// The tag name, prefix included.
    pub name: &'a str,
    /// The Version the tag names.
    pub version: &'a Version,
    /// Whether nothing was actually created.
    pub dry_run: bool,
}

impl Report for Tag<'_> {
    fn human(&self) -> String {
        let verb = if self.dry_run { "would tag" } else { "tagged" };
        format!("{verb} {}\n", self.name)
    }

    fn json(&self) -> String {
        Json::object(vec![
            ("tag", Json::from(self.name)),
            ("version", Json::from(self.version)),
            ("dry_run", Json::from(self.dry_run)),
        ])
        .to_document()
    }

    fn version(&self) -> String {
        self.version.to_string()
    }
}

/// A JSON value, rendered with a stable two-space indentation.
///
/// The workspace carries no JSON dependency and momo's reports are small, so the
/// documents are assembled deliberately here rather than pulled in. Object key
/// order is preserved, so the output is byte-stable.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Json {
    /// `null`.
    Null,
    /// A JSON boolean.
    Bool(bool),
    /// A JSON string.
    String(String),
    /// A JSON array.
    Array(Vec<Json>),
    /// A JSON object; key order is preserved.
    Object(Vec<(String, Json)>),
}

impl Json {
    /// A JSON object with the given keys and values, in order.
    fn object(entries: Vec<(&str, Json)>) -> Self {
        Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        )
    }

    /// Render this value as a pretty JSON document, newline-terminated.
    fn to_document(&self) -> String {
        let mut out = String::new();
        self.render(0, &mut out);
        out.push('\n');
        out
    }

    /// Append this value to `out`, indenting nested values `depth` levels deep.
    fn render(&self, depth: usize, out: &mut String) {
        match self {
            Json::Null => out.push_str("null"),
            Json::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
            Json::String(text) => {
                out.push('"');
                escape_into(text, out);
                out.push('"');
            }
            Json::Array(items) if items.is_empty() => out.push_str("[]"),
            Json::Array(items) => {
                out.push_str("[\n");
                for (index, item) in items.iter().enumerate() {
                    indent(depth + 1, out);
                    item.render(depth + 1, out);
                    comma(index, items.len(), out);
                }
                indent(depth, out);
                out.push(']');
            }
            Json::Object(entries) if entries.is_empty() => out.push_str("{}"),
            Json::Object(entries) => {
                out.push_str("{\n");
                for (index, (key, value)) in entries.iter().enumerate() {
                    indent(depth + 1, out);
                    out.push('"');
                    escape_into(key, out);
                    out.push_str("\": ");
                    value.render(depth + 1, out);
                    comma(index, entries.len(), out);
                }
                indent(depth, out);
                out.push('}');
            }
        }
    }
}

impl From<&str> for Json {
    fn from(value: &str) -> Self {
        Json::String(value.to_owned())
    }
}

impl From<String> for Json {
    fn from(value: String) -> Self {
        Json::String(value)
    }
}

impl From<bool> for Json {
    fn from(value: bool) -> Self {
        Json::Bool(value)
    }
}

impl From<&Version> for Json {
    fn from(value: &Version) -> Self {
        Json::String(value.to_string())
    }
}

impl From<&Path> for Json {
    fn from(value: &Path) -> Self {
        Json::String(value.display().to_string())
    }
}

/// Append a comma unless `index` is the last of `len` entries, then a newline.
fn comma(index: usize, len: usize, out: &mut String) {
    if index + 1 < len {
        out.push(',');
    }
    out.push('\n');
}

/// Append two spaces per `depth` level of indentation.
fn indent(depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
}

/// Append `text` to `out`, escaped for a JSON string.
fn escape_into(text: &str, out: &mut String) {
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            control if control < '\u{20}' => out.push_str(&format!("\\u{:04x}", control as u32)),
            other => out.push(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    /// A `Cli` with every global flag at its default.
    fn cli() -> Cli {
        Cli {
            command: crate::cli::Command::Status { check: false },
            dry_run: false,
            format: Format::Human,
            version_only: false,
            output: None,
            package: None,
            all: false,
        }
    }

    /// A status report with one `add` Change at the bootstrap baseline.
    fn status<'a>(changes: &'a [Change]) -> Status<'a> {
        let current = Version::new(0, 0, 0);
        let next = momo_core::version::release_version(changes, &current);
        Status {
            package: "momo",
            current,
            next,
            changes,
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
      "current": "0.0.0",
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
        assert!(json.contains("\"changes\": []"), "{json}");
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
    fn release_json_has_the_documented_shape() {
        let version = version("0.1.0");
        let report = Release {
            version: &version,
            changelog: Path::new("/repo/CHANGELOG.md"),
            version_file: Some(Path::new("/repo/.momo/version")),
            consumed: &[PathBuf::from("/repo/.momo/changes/add-a-thing.md")],
            verb: Verb::Release,
            dry_run: false,
        };

        assert_eq!(
            report.json(),
            r#"{
  "version": "0.1.0",
  "changelog": "/repo/CHANGELOG.md",
  "version_file": "/repo/.momo/version",
  "consumed": [
    "/repo/.momo/changes/add-a-thing.md"
  ],
  "dry_run": false
}
"#
        );
    }

    #[test]
    fn release_json_reports_a_missing_version_file_as_null() {
        let version = version("0.1.0");
        let report = Release {
            version: &version,
            changelog: Path::new("/repo/CHANGELOG.md"),
            version_file: None,
            consumed: &[],
            verb: Verb::Candidate,
            dry_run: true,
        };

        let json = report.json();
        assert!(json.contains("\"version_file\": null"), "{json}");
        assert!(json.contains("\"consumed\": []"), "{json}");
        assert!(json.contains("\"dry_run\": true"), "{json}");
    }

    #[test]
    fn release_human_names_the_verb_and_what_it_wrote() {
        let version = version("0.1.0");
        let report = Release {
            version: &version,
            changelog: Path::new("/repo/CHANGELOG.md"),
            version_file: Some(Path::new("/repo/.momo/version")),
            consumed: &[PathBuf::from("/repo/.momo/changes/add-a-thing.md")],
            verb: Verb::Release,
            dry_run: false,
        };

        assert_eq!(
            report.human(),
            "released 0.1.0\nwrote /repo/CHANGELOG.md\nwrote /repo/.momo/version\nremoved /repo/.momo/changes/add-a-thing.md\n"
        );
    }

    #[test]
    fn release_human_speaks_in_the_conditional_under_dry_run() {
        let version = version("0.1.0-rc.1");
        let report = Release {
            version: &version,
            changelog: Path::new("/repo/CHANGELOG.md"),
            version_file: None,
            consumed: &[],
            verb: Verb::Candidate,
            dry_run: true,
        };

        assert_eq!(
            report.human(),
            "would cut 0.1.0-rc.1\nwould write /repo/CHANGELOG.md\n"
        );
    }

    #[test]
    fn release_version_only_is_the_minted_version() {
        let version = version("1.2.3");
        let report = Release {
            version: &version,
            changelog: Path::new("/repo/CHANGELOG.md"),
            version_file: None,
            consumed: &[],
            verb: Verb::Release,
            dry_run: false,
        };

        assert_eq!(report.version(), "1.2.3");
    }

    #[test]
    fn tag_json_has_the_documented_shape() {
        let version = version("0.1.0");
        let report = Tag {
            name: "v0.1.0",
            version: &version,
            dry_run: false,
        };

        assert_eq!(
            report.json(),
            "{\n  \"tag\": \"v0.1.0\",\n  \"version\": \"0.1.0\",\n  \"dry_run\": false\n}\n"
        );
    }

    #[test]
    fn tag_human_speaks_in_the_conditional_under_dry_run() {
        let version = version("0.1.0");
        assert_eq!(
            Tag {
                name: "v0.1.0",
                version: &version,
                dry_run: true,
            }
            .human(),
            "would tag v0.1.0\n"
        );
    }

    #[test]
    fn version_only_wins_over_the_format() {
        let mut cli = cli();
        cli.version_only = true;
        cli.format = Format::Json;

        assert_eq!(render(&cli, &status(&[])), "0.0.0\n");
    }

    #[test]
    fn the_format_selects_the_rendering() {
        let mut cli = cli();
        cli.format = Format::Json;

        assert!(render(&cli, &status(&[])).starts_with("{\n"));
    }

    #[test]
    fn emit_writes_to_the_output_path_instead_of_stdout() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("status.json");
        let mut cli = cli();
        cli.format = Format::Json;
        cli.output = Some(path.clone());

        emit(&cli, &status(&[])).expect("emit");

        let written = fs::read_to_string(&path).expect("read the written file");
        assert_eq!(written, status(&[]).json());
    }
}
