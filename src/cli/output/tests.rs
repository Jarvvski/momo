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
        manifest: vec![PathBuf::from("/repo/Cargo.toml")],
        skipped: Vec::new(),
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
      "manifest": [
        "/repo/Cargo.toml"
      ],
      "skipped": [],
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
fn release_json_reports_a_missing_manifest_as_an_empty_array() {
    let report = Release {
        verb: Verb::Release,
        dry_run: false,
        packages: vec![ReleasePackage {
            manifest: Vec::new(),
            ..package_entry("momo", "0.1.0")
        }],
    };

    assert!(
        report.json().contains("\"manifest\": []"),
        "{}",
        report.json()
    );
}

#[test]
fn release_json_reports_a_skipped_manifest_with_its_reason() {
    let report = Release {
        verb: Verb::Release,
        dry_run: false,
        packages: vec![ReleasePackage {
            manifest: Vec::new(),
            skipped: vec![SkippedManifest {
                path: PathBuf::from("/repo/pyproject.toml"),
                reason: "dynamic version (hatch-vcs)".to_owned(),
            }],
            ..package_entry("momo", "0.1.0")
        }],
    };

    let json = report.json();
    assert!(json.contains("\"skipped\""), "{json}");
    assert!(
        json.contains("\"path\": \"/repo/pyproject.toml\""),
        "{json}"
    );
    assert!(
        json.contains("\"reason\": \"dynamic version (hatch-vcs)\""),
        "{json}"
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
            manifest: Vec::new(),
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
