//! The Kind and the Change: what a recorded unit of work is.

use std::fmt;
use std::str::FromStr;

use crate::domain::diagnostic::impl_coded;
use crate::domain::slug::slugify;

/// The classification of a Change — one of `break`, `add`, `fix`.
///
/// The variants are ordered from lowest to highest severity, so the highest
/// Kind in a Release is `kinds.into_iter().max()`: `break` > `add` > `fix`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// A bug fix; bumps PATCH.
    Fix,
    /// An addition; bumps MINOR.
    Add,
    /// A breaking change; bumps MINOR at MAJOR 0 and MAJOR at MAJOR ≥ 1.
    Break,
}

impl Kind {
    /// Every Kind, highest severity first — the order of Changelog sections.
    pub const ALL: [Kind; 3] = [Kind::Break, Kind::Add, Kind::Fix];

    /// The Changelog section heading for this Kind.
    pub fn heading(self) -> &'static str {
        match self {
            Kind::Break => "Breaking",
            Kind::Add => "Added",
            Kind::Fix => "Fixed",
        }
    }

    /// The Kind a Changelog section heading names, if any.
    pub fn from_heading(heading: &str) -> Option<Self> {
        match heading.trim() {
            "Breaking" => Some(Kind::Break),
            "Added" => Some(Kind::Add),
            "Fixed" => Some(Kind::Fix),
            _ => None,
        }
    }
}

/// The Keep-a-Changelog classification headings momo cannot read.
///
/// These are Keep-a-Changelog 1.1.0's categories minus the three momo reads
/// (`Added` and `Fixed` map to [`Kind::Add`] and [`Kind::Fix`]; `Breaking` is
/// momo's own heading, not a Keep-a-Changelog one). The categories describe
/// *what kind* of change happened, not its severity — `Changed` can break or
/// not — so momo has no honest Bump to derive from one. A bullet beneath one is
/// still Unreleased content momo is failing to version.
pub const UNREAD_HEADINGS: [&str; 4] = ["Changed", "Deprecated", "Removed", "Security"];

/// Whether `heading` is a Keep-a-Changelog heading momo cannot read.
///
/// The set is closed: a heading outside it and outside [`Kind::from_heading`]
/// (say `### Internal`) is not a Keep-a-Changelog classification, so it stays
/// foreign — preserved and ignored, the documented escape hatch.
pub fn unread_heading(heading: &str) -> bool {
    UNREAD_HEADINGS.contains(&heading.trim())
}

impl fmt::Display for Kind {
    /// The verb that names this Kind.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Break => "break",
            Kind::Add => "add",
            Kind::Fix => "fix",
        })
    }
}

/// The error produced when a Kind name is not `break`, `add`, or `fix`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown kind `{value}`; expected one of: break, add, fix")]
pub struct UnknownKind {
    /// The unrecognised name.
    pub value: String,
}

impl FromStr for Kind {
    type Err = UnknownKind;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "break" => Ok(Kind::Break),
            "add" => Ok(Kind::Add),
            "fix" => Ok(Kind::Fix),
            other => Err(UnknownKind {
                value: other.to_owned(),
            }),
        }
    }
}

/// The error produced when a Change's text is empty once normalised.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("a change needs some text")]
pub struct EmptyChange;

impl_coded! {
    UnknownKind {
        UnknownKind { .. } => ("momo.change.unknown-kind", fix = "use `break`, `add` or `fix`"),
    }
}

impl_coded! {
    EmptyChange {
        EmptyChange => ("momo.change.empty-text", fix = "give the change some text"),
    }
}

/// One recorded unit of work, classified by exactly one Kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// The Kind classifying this Change.
    pub kind: Kind,
    /// The Change's text, as recorded.
    pub text: String,
}

impl Change {
    /// Record a Change of `kind` with `text`, trusting `text` verbatim.
    ///
    /// The parsing path uses this: the text it recovers from a Changelog is
    /// already a single line. Text from a user goes through [`Self::try_new`].
    pub fn new(kind: Kind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
        }
    }

    /// Record a Change of `kind` with `text`, normalising and validating it.
    ///
    /// `text` is trimmed and every run of whitespace — newlines and tabs
    /// included — collapses to a single space, so a Change can never forge a
    /// second Changelog line or heading. Whitespace-only text is rejected:
    /// there is nothing to record, and it would otherwise mint a Release.
    pub fn try_new(kind: Kind, text: &str) -> Result<Self, EmptyChange> {
        let text = collapse_whitespace(text);
        if text.is_empty() {
            return Err(EmptyChange);
        }
        Ok(Self { kind, text })
    }

    /// The Fragment file name for this Change: `<kind>-<slug>.md`.
    pub fn fragment_file_name(&self) -> String {
        format!("{}-{}.md", self.kind, slugify(&self.text))
    }
}

/// Trim `text` and collapse every run of whitespace to a single space.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_are_ordered_by_severity() {
        assert!(Kind::Break > Kind::Add);
        assert!(Kind::Add > Kind::Fix);
        assert_eq!(
            [Kind::Fix, Kind::Break, Kind::Add].into_iter().max(),
            Some(Kind::Break)
        );
    }

    #[test]
    fn kind_names_round_trip() {
        for kind in Kind::ALL {
            assert_eq!(kind.to_string().parse::<Kind>(), Ok(kind));
        }
        assert_eq!(
            "nonsense".parse::<Kind>(),
            Err(UnknownKind {
                value: "nonsense".to_owned()
            })
        );
    }

    #[test]
    fn headings_map_to_their_kind() {
        assert_eq!(Kind::from_heading("Breaking"), Some(Kind::Break));
        assert_eq!(Kind::from_heading("Added"), Some(Kind::Add));
        assert_eq!(Kind::from_heading("Fixed"), Some(Kind::Fix));
        assert_eq!(Kind::from_heading("Changed"), None);
    }

    #[test]
    fn the_heading_strings_are_pinned() {
        // The changelog shape is an asserted opinion, not config: `Breaking` is
        // deliberate where Keep a Changelog says `Changed`. Pinned so a rename is
        // a conscious, reviewed change (see docs/opinions.md).
        assert_eq!(Kind::Break.heading(), "Breaking");
        assert_eq!(Kind::Add.heading(), "Added");
        assert_eq!(Kind::Fix.heading(), "Fixed");
    }

    #[test]
    fn unread_heading_matches_only_the_keep_a_changelog_categories_momo_cannot_read() {
        for heading in UNREAD_HEADINGS {
            assert!(unread_heading(heading), "`{heading}` should be unread");
        }
        // Whitespace is trimmed, so a padded heading still matches.
        assert!(unread_heading("  Changed "));
        // The three momo reads, and anything outside the closed set, do not.
        for heading in ["Breaking", "Added", "Fixed", "Internal", "changed"] {
            assert!(!unread_heading(heading), "`{heading}` should not be unread");
        }
    }

    #[test]
    fn fragment_file_name_encodes_kind_and_slug() {
        let change = Change::new(Kind::Add, "`momo status --format json` for CI");
        assert_eq!(
            change.fragment_file_name(),
            "add-momo-status-format-json-for-ci.md"
        );
    }

    #[test]
    fn try_new_rejects_empty_or_whitespace_only_text() {
        assert_eq!(Change::try_new(Kind::Add, ""), Err(EmptyChange));
        assert_eq!(Change::try_new(Kind::Fix, " \n\t "), Err(EmptyChange));
    }

    #[test]
    fn try_new_trims_and_collapses_whitespace_runs() {
        assert_eq!(
            Change::try_new(Kind::Add, "  a \n b\t\tc  ").expect("some text"),
            Change::new(Kind::Add, "a b c")
        );
    }

    #[test]
    fn try_new_flattens_an_injected_section_heading_onto_one_line() {
        let change = Change::try_new(Kind::Add, "x\n## [9.9.9] - 2000-01-01").expect("some text");
        assert_eq!(change.text, "x ## [9.9.9] - 2000-01-01");
    }
}
