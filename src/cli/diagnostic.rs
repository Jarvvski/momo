//! Rendering a failed run as a stable, structured error.
//!
//! momo keeps `anyhow` at its boundary: the `.context(...)` layers are the
//! *why*, and the leaf error is the *what*. [`diagnose`] walks that chain once
//! and finds the first error carrying a [`Coded`] code, so a wrapper (an
//! internal variant) does not hide the actionable error beneath it. The code,
//! the leaf message, the fix and the outer context are then rendered for a
//! human on stderr, or as one JSON document on stdout under `--format json`.

use std::error::Error;

use anyhow::Error as AnyhowError;
use serde::Serialize;

use crate::cli::error::CliError;
use crate::domain::change::{EmptyChange, UnknownKind};
use crate::domain::changelog::ChangelogError;
use crate::domain::config::ConfigError;
use crate::domain::diagnostic::{Coded, UNCLASSIFIED};
use crate::domain::manifest::ManifestError;
use crate::domain::taskenv::TaskEnvError;
use crate::domain::vcs::VcsError;
use crate::domain::version::{RcFormatError, VersionError};
use crate::domain::workspace::{SelectionError, WorkspaceError};

/// The base URL every code's documentation anchor hangs off.
const DOCS_BASE: &str = "https://github.com/Jarvvski/momo/blob/main/docs/errors.md";

/// A rendered error: the stable code, the leaf message, the fix, a doc URL and
/// the outer context that led to it.
#[derive(Debug)]
pub struct Diagnostic {
    /// The stable code, or [`UNCLASSIFIED`].
    code: &'static str,
    /// The leaf error's message.
    message: String,
    /// What the user can do about it, when there is something.
    fix: Option<String>,
    /// Where to read more.
    url: Option<String>,
    /// The outer context, outermost first.
    context: Vec<String>,
    /// The whole chain on one line — the human explanation.
    chain: String,
}

/// Classify `err` against its chain.
pub fn diagnose(err: &AnyhowError) -> Diagnostic {
    let mut context = Vec::new();
    let mut found: Option<(&dyn Coded, String)> = None;
    for cause in err.chain() {
        match coded(cause) {
            Some(coded) if coded.code() != UNCLASSIFIED => {
                found = Some((coded, cause.to_string()));
                break;
            }
            _ => context.push(cause.to_string()),
        }
    }

    let (code, message, fix, url) = match found {
        Some((coded, message)) => (
            coded.code(),
            message,
            coded.fix(),
            coded.url().map(str::to_owned),
        ),
        None => {
            // Nothing in the chain is classified: the root cause is the
            // message and everything above it is context.
            let message = context.pop().unwrap_or_else(|| err.to_string());
            (UNCLASSIFIED, message, None, None)
        }
    };
    let url = url.or_else(|| Some(format!("{DOCS_BASE}#{}", code.replace('.', "-"))));

    Diagnostic {
        code,
        message,
        fix,
        url,
        context,
        chain: format!("{err:#}"),
    }
}

impl Diagnostic {
    /// The human rendering: `error[code]: <chain>`, plus an indented `fix:`.
    pub fn human(&self) -> String {
        let mut out = format!("error[{}]: {}", self.code, self.chain);
        if let Some(fix) = &self.fix {
            out.push_str("\n  fix: ");
            out.push_str(fix);
        }
        out
    }

    /// The JSON rendering, newline-terminated: `{ "error": { … } }`.
    pub fn json(&self) -> String {
        let doc = Envelope {
            error: ErrorDoc {
                code: self.code,
                message: &self.message,
                fix: self.fix.as_deref(),
                url: self.url.as_deref(),
                context: &self.context,
            },
        };
        let mut json = serde_json::to_string_pretty(&doc)
            .expect("a diagnostic holds only owned data, so it always serializes");
        json.push('\n');
        json
    }
}

/// The `error` envelope: the one key a JSON consumer reads.
#[derive(Serialize)]
struct Envelope<'a> {
    /// The error document.
    error: ErrorDoc<'a>,
}

/// The documented error shape.
#[derive(Serialize)]
struct ErrorDoc<'a> {
    /// The stable code.
    code: &'a str,
    /// The leaf message.
    message: &'a str,
    /// The fix, when there is one.
    fix: Option<&'a str>,
    /// Documentation for the code.
    url: Option<&'a str>,
    /// The outer context, outermost first.
    context: &'a [String],
}

/// Every error type that implements [`Coded`].
///
/// The chain probe and the docs drift test both read this one list, so a new
/// `Coded` type is registered — and checked against `docs/errors.md` — by being
/// added here.
macro_rules! for_each_coded {
    ($callback:ident) => {
        $callback! {
            CliError,
            ChangelogError,
            RcFormatError,
            VersionError,
            VcsError,
            TaskEnvError,
            ConfigError,
            ManifestError,
            SelectionError,
            WorkspaceError,
            UnknownKind,
            EmptyChange,
        }
    };
}

/// The code-carrying error at the head of a chain element, when it has one.
fn coded<'a>(err: &'a (dyn Error + 'static)) -> Option<&'a dyn Coded> {
    macro_rules! probe {
        ($($ty:ty),* $(,)?) => {
            $(
                if let Some(found) = err.downcast_ref::<$ty>() {
                    let found: &dyn Coded = found;
                    return Some(found);
                }
            )*
        };
    }

    for_each_coded!(probe);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every code momo can report, deduplicated and sorted.
    fn every_code() -> Vec<&'static str> {
        macro_rules! collect {
            ($($ty:ty),* $(,)?) => {{
                let mut codes: Vec<&'static str> = Vec::new();
                $( codes.extend_from_slice(<$ty>::CODES); )*
                codes
            }};
        }

        let mut codes = for_each_coded!(collect);
        codes.push(UNCLASSIFIED);
        codes.sort_unstable();
        codes.dedup();
        codes
    }

    /// `docs/errors.md`, read from the crate root.
    fn errors_doc() -> String {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/errors.md");
        std::fs::read_to_string(path).expect("docs/errors.md is readable")
    }

    #[test]
    fn every_code_has_a_docs_section() {
        let docs = errors_doc();
        for code in every_code() {
            assert!(
                docs.contains(&format!("### `{code}`")),
                "docs/errors.md has no section for `{code}`",
            );
        }
    }

    #[test]
    fn every_docs_section_names_a_code() {
        let docs = errors_doc();
        let codes = every_code();
        let documented: Vec<&str> = docs
            .lines()
            .filter_map(|line| line.strip_prefix("### `")?.strip_suffix('`'))
            .collect();
        assert!(
            !documented.is_empty(),
            "docs/errors.md has no `### `code`` sections",
        );
        for code in documented {
            assert!(
                codes.contains(&code),
                "docs/errors.md documents `{code}`, which no error reports",
            );
        }
    }

    #[test]
    fn a_coded_error_is_classified_with_its_fix() {
        let err = AnyhowError::from(CliError::NothingUnreleased);

        let diagnostic = diagnose(&err);

        assert_eq!(diagnostic.code, "momo.release.nothing-unreleased");
        assert_eq!(diagnostic.message, "nothing unreleased");
        assert!(
            diagnostic
                .fix
                .as_deref()
                .is_some_and(|fix| fix.contains("momo add")),
            "{diagnostic:?}",
        );
        assert_eq!(
            diagnostic.url.as_deref(),
            Some(
                "https://github.com/Jarvvski/momo/blob/main/docs/errors.md#momo-release-nothing-unreleased"
            ),
        );
        assert_eq!(
            diagnostic.human(),
            "error[momo.release.nothing-unreleased]: nothing unreleased\n  \
             fix: record a change with `momo add`, `momo fix` or `momo break`",
        );
    }

    #[test]
    fn the_context_becomes_the_explanation_and_the_leaf_the_message() {
        let err =
            AnyhowError::from(CliError::NothingUnreleased).context("validating the configuration");

        let diagnostic = diagnose(&err);

        assert_eq!(diagnostic.code, "momo.release.nothing-unreleased");
        assert_eq!(diagnostic.message, "nothing unreleased");
        assert_eq!(diagnostic.context, ["validating the configuration"]);
        assert_eq!(
            diagnostic.human(),
            "error[momo.release.nothing-unreleased]: validating the configuration: nothing unreleased\n  \
             fix: record a change with `momo add`, `momo fix` or `momo break`",
        );
    }

    #[test]
    fn a_wrapper_variant_does_not_hide_the_coded_error_beneath_it() {
        let err = AnyhowError::from(VersionError::Vcs(VcsError::TagAlreadyExists {
            tag: "v1.0.0".to_owned(),
        }));

        let diagnostic = diagnose(&err);

        assert_eq!(diagnostic.code, "momo.vcs.tag-exists");
        assert_eq!(diagnostic.message, "tag `v1.0.0` already exists");
        assert!(
            diagnostic.context[0].contains("reading the latest tag"),
            "{diagnostic:?}",
        );
    }

    #[test]
    fn an_unclassified_error_falls_through_to_the_root_cause() {
        let err = AnyhowError::from(std::io::Error::other("boom")).context("doing a thing");

        let diagnostic = diagnose(&err);

        assert_eq!(diagnostic.code, UNCLASSIFIED);
        assert_eq!(diagnostic.message, "boom");
        assert_eq!(diagnostic.context, ["doing a thing"]);
        assert!(diagnostic.fix.is_none());
        assert_eq!(
            diagnostic.url.as_deref(),
            Some(
                "https://github.com/Jarvvski/momo/blob/main/docs/errors.md#momo-error-unclassified"
            ),
        );
    }

    #[test]
    fn json_is_one_document_with_the_documented_keys() {
        let err = AnyhowError::from(CliError::NoVcs);

        let value: serde_json::Value =
            serde_json::from_str(&diagnose(&err).json()).expect("one JSON document");

        let error = value["error"].as_object().expect("an `error` object");
        let mut keys: Vec<&str> = error.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, ["code", "context", "fix", "message", "url"]);
        assert_eq!(error["code"], "momo.tag.no-vcs");
        assert_eq!(error["message"], "no VCS detected — nothing to tag");
        assert_eq!(error["context"], serde_json::json!([]));
    }
}
