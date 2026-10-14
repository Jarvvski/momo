//! Property tests for the changelog preserve model (ADR 0003).
//!
//! The contract is that momo edits only the spans it owns — the Unreleased body,
//! its own reference links, and the release headings it inserts — and passes
//! everything else through byte-for-byte. These tests generate documents
//! carrying foreign content (`*` bullets, HTML comments, `### Changed` sections,
//! user-authored reference-link footers) across both line endings, and assert
//! the invariants the model exists to keep. A failure here is a counterexample,
//! not a flake: proptest shrinks it to a minimal document.

use crate::domain::change::{Change, Kind};
use crate::domain::changelog::{Changelog, ChangelogError, Links};
use proptest::prelude::*;
use semver::Version;
use time::Date;

/// A generated document, and the foreign lines a release must not drop.
#[derive(Debug)]
struct Doc {
    /// The document text, with the generated line ending.
    text: String,
    /// The lines momo does not own; every one must survive a release.
    foreign: Vec<String>,
}

/// Links with no remote: momo regenerates no footer, so every line passes
/// through untouched.
fn no_links() -> Links<'static> {
    Links {
        prefix: "v",
        remote: None,
    }
}

/// A fixed date for a release heading.
fn date(year: i32, month: u8, day: u8) -> Date {
    Date::from_calendar_date(year, time::Month::try_from(month).expect("a month"), day)
        .expect("a valid date")
}

/// A single-line change text: non-empty, and free of newlines so it cannot
/// forge a second document line.
fn change_text() -> impl Strategy<Value = String> {
    "[A-Za-z0-9][A-Za-z0-9 .:_#-]{0,30}"
}

/// A Kind, drawn from all three.
fn kind() -> impl Strategy<Value = Kind> {
    prop::sample::select(vec![Kind::Break, Kind::Add, Kind::Fix])
}

/// A foreign `###` heading: one momo does not interpret as a Kind.
fn foreign_heading() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "### Changed".to_owned(),
        "### Removed".to_owned(),
        "### Deprecated".to_owned(),
        "### Security".to_owned(),
    ])
}

/// A user-authored reference-link definition.
fn reference_link() -> impl Strategy<Value = String> {
    (
        "[A-Za-z][A-Za-z ]{0,12}",
        "[A-Za-z0-9][A-Za-z0-9 ./_-]{0,16}",
    )
        .prop_map(|(label, path)| format!("[{label}]: https://example.com/{path}"))
}

/// One line of foreign content that may appear anywhere in a document.
fn foreign_line() -> impl Strategy<Value = String> {
    prop_oneof![
        3 => change_text().prop_map(|text| format!("- {text}")),
        2 => change_text().prop_map(|text| format!("* {text}")),
        2 => change_text().prop_map(|text| format!("<!-- {text} -->")),
        2 => foreign_heading(),
        2 => reference_link(),
    ]
}

/// A line above the first `##` section.
fn preamble_line() -> impl Strategy<Value = String> {
    prop_oneof![
        2 => change_text(),
        1 => change_text().prop_map(|text| format!("# {text}")),
    ]
}

/// A released `## [version] - date` section and its body.
fn released_section() -> impl Strategy<Value = (String, Vec<String>)> {
    (
        prop::sample::select(vec![
            "1.0.0".to_owned(),
            "0.2.0".to_owned(),
            "2.3.4".to_owned(),
            "0.1.0-rc.1".to_owned(),
        ]),
        prop::collection::vec(foreign_line(), 0..4),
    )
        .prop_map(|(version, body)| (format!("## [{version}] - 2026-01-01"), body))
}

/// A document with exactly one Unreleased section, placed anywhere among the
/// released sections, and foreign content around it.
fn document() -> impl Strategy<Value = Doc> {
    (
        prop::collection::vec(preamble_line(), 0..3),
        prop::collection::vec(foreign_line(), 0..5),
        prop::collection::vec(released_section(), 0..3),
        prop::collection::vec(reference_link(), 0..3),
        any::<bool>(),
        0usize..4,
    )
        .prop_map(|(preamble, unreleased, released, footer, crlf, position)| {
            assemble(preamble, unreleased, released, footer, crlf, position)
        })
}

/// Push a line, recording it as foreign content that must survive.
fn push_line(lines: &mut Vec<String>, foreign: &mut Vec<String>, line: String) {
    foreign.push(line.clone());
    lines.push(line);
}

/// Push a blank separator unless the buffer is empty or already blank.
fn blank(lines: &mut Vec<String>) {
    if lines.last().is_some_and(|line| !line.is_empty()) {
        lines.push(String::new());
    }
}

/// Push the Unreleased section: its heading is momo's to consume, so only the
/// body lines are recorded as foreign.
fn push_unreleased(lines: &mut Vec<String>, foreign: &mut Vec<String>, body: Vec<String>) {
    blank(lines);
    lines.push("## [Unreleased]".to_owned());
    blank(lines);
    for line in body {
        push_line(lines, foreign, line);
    }
}

/// Lay the generated pieces out as a Keep-a-Changelog document.
fn assemble(
    preamble: Vec<String>,
    unreleased: Vec<String>,
    released: Vec<(String, Vec<String>)>,
    footer: Vec<String>,
    crlf: bool,
    position: usize,
) -> Doc {
    let mut lines = Vec::new();
    let mut foreign = Vec::new();

    for line in preamble {
        push_line(&mut lines, &mut foreign, line);
    }

    let position = position.min(released.len());
    let mut unreleased = Some(unreleased);
    for (index, (heading, body)) in released.into_iter().enumerate() {
        if index == position
            && let Some(body) = unreleased.take()
        {
            push_unreleased(&mut lines, &mut foreign, body);
        }
        blank(&mut lines);
        push_line(&mut lines, &mut foreign, heading);
        blank(&mut lines);
        for line in body {
            push_line(&mut lines, &mut foreign, line);
        }
    }
    if let Some(body) = unreleased.take() {
        push_unreleased(&mut lines, &mut foreign, body);
    }

    if !footer.is_empty() {
        blank(&mut lines);
        for line in footer {
            push_line(&mut lines, &mut foreign, line);
        }
    }

    let ending = if crlf { "\r\n" } else { "\n" };
    let mut text = lines.join(ending);
    text.push_str(ending);
    Doc { text, foreign }
}

/// Assert every foreign line survived into `rendered`.
fn assert_foreign_survives(
    rendered: &str,
    foreign: &[String],
    action: &str,
) -> Result<(), TestCaseError> {
    let rendered_lines: Vec<&str> = rendered.lines().collect();
    for line in foreign {
        prop_assert!(
            rendered_lines.contains(&line.as_str()),
            "{action} dropped the foreign line {line:?}\n--- rendered ---\n{rendered}"
        );
    }
    Ok(())
}

proptest! {
    /// `parse` → `render` is a fixed point: rendering a parsed document and
    /// parsing it again reproduces the same document.
    #[test]
    fn parse_render_is_a_fixed_point_for_a_generated_document(doc in document()) {
        let changelog = Changelog::parse(&doc.text).expect("a generated document parses");
        let first = changelog.render();
        let second = Changelog::parse(&first)
            .expect("a rendered changelog parses")
            .render();
        prop_assert_eq!(first, second);
    }

    /// The fixed-point property holds for arbitrary text too, not just the
    /// structured generator: whenever `parse` succeeds, a second pass changes
    /// nothing.
    #[test]
    fn parse_render_is_a_fixed_point_for_arbitrary_text(text in any::<String>()) {
        let Ok(changelog) = Changelog::parse(&text) else {
            return Ok(());
        };
        let first = changelog.render();
        let second = Changelog::parse(&first)
            .expect("a rendered changelog parses")
            .render();
        prop_assert_eq!(first, second);
    }

    /// A release never drops a line momo does not own.
    #[test]
    fn a_release_never_drops_a_foreign_line(doc in document()) {
        let mut changelog = Changelog::parse(&doc.text).expect("a generated document parses");
        let changes = changelog.unreleased();
        changelog.release(
            &Version::new(9, 9, 9),
            date(2026, 10, 13),
            &changes,
            no_links(),
        );

        let rendered = changelog.render();
        assert_foreign_survives(&rendered, &doc.foreign, "release")?;
    }

    /// Recording never drops a line momo does not own either.
    #[test]
    fn recording_never_drops_a_foreign_line(
        doc in document(),
        kind in kind(),
        text in change_text(),
    ) {
        let mut changelog = Changelog::parse(&doc.text).expect("a generated document parses");
        let change = Change::try_new(kind, &text).expect("the generated text is non-empty");
        changelog.record(change);

        let rendered = changelog.render();
        assert_foreign_survives(&rendered, &doc.foreign, "recording")?;
    }

    /// `record` then `unreleased` returns the recorded Change.
    #[test]
    fn a_recorded_change_round_trips_through_unreleased(kind in kind(), text in change_text()) {
        let change = Change::try_new(kind, &text).expect("the generated text is non-empty");
        let mut changelog = Changelog::empty();
        changelog.record(change.clone());
        prop_assert_eq!(changelog.unreleased(), vec![change]);
    }

    /// Every recorded Change reaches `unreleased`, and nothing extra appears.
    #[test]
    fn every_recorded_change_reaches_unreleased(
        changes in prop::collection::vec((kind(), change_text()), 0..5),
    ) {
        let mut changelog = Changelog::empty();
        let mut expected = Vec::new();
        for (kind, text) in changes {
            let change = Change::try_new(kind, &text).expect("the generated text is non-empty");
            changelog.record(change.clone());
            expected.push(change);
        }

        let unreleased = changelog.unreleased();
        for change in &expected {
            prop_assert!(
                unreleased.contains(change),
                "the recorded change {change:?} was not returned by `unreleased`: {unreleased:?}"
            );
        }
        prop_assert_eq!(unreleased.len(), expected.len());
    }

    /// Arbitrary, including malformed, input never panics: `parse` either
    /// succeeds or fails with a `ChangelogError` naming a real line. proptest
    /// catches a panic and reports it as a failure, so reaching the end of this
    /// body is the proof that none occurred.
    #[test]
    fn parsing_arbitrary_input_never_panics(text in any::<String>()) {
        match Changelog::parse(&text) {
            Ok(_) => {}
            Err(ChangelogError::DuplicateUnreleased { line }) => prop_assert!(line >= 1),
            Err(ChangelogError::UnreadSection { .. }) => {
                prop_assert!(false, "parse cannot produce an unread-section error")
            }
        }
    }
}
