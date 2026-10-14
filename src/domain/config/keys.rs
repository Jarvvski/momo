//! The recognised `momo.toml` keys, their defaults and their deserialisation.

use std::str::FromStr;
use std::time::Duration;

use toml_edit::{DocumentMut, Item};

use crate::domain::remote::Remote;

use super::paths::{parse_packages, parse_path};
use super::{ConfigError, ConfigOverrides, Recording, UnknownValue};

/// The top-level keys a `momo.toml` may set.
const KNOWN_KEYS: [&str; 9] = [
    "recording",
    "changelog",
    "changes_dir",
    "tag_prefix",
    "remote",
    "task_env",
    "rc_format",
    "command_timeout",
    "packages",
];

/// The keys a `[packages.<id>]` table may set.
pub(super) const KNOWN_PACKAGE_KEYS: [&str; 3] = ["identity", "slug", "manifest"];

impl FromStr for ConfigOverrides {
    type Err = ConfigError;

    /// Parse overrides from `momo.toml` text.
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let doc: DocumentMut = input.parse()?;
        reject_unknown_keys(doc.as_table().iter().map(|(key, _)| key), &KNOWN_KEYS, "")?;
        Ok(Self {
            recording: parse_scalar::<Recording>(&doc, "recording")?,
            changelog: parse_path(&doc, "changelog")?,
            changes_dir: parse_path(&doc, "changes_dir")?,
            tag_prefix: parse_string(&doc, "tag_prefix")?,
            remote: parse_remote(&doc, "remote")?,
            task_env: parse_string(&doc, "task_env")?,
            rc_format: parse_string(&doc, "rc_format")?,
            command_timeout: parse_seconds_key(&doc, "command_timeout")?,
            packages: parse_packages(&doc)?,
        })
    }
}

/// Parse a scalar key from the document into `T`.
fn parse_scalar<T: FromStr<Err = UnknownValue>>(
    doc: &DocumentMut,
    key: &str,
) -> Result<Option<T>, ConfigError> {
    string_at(doc, key)?
        .map(|value| parse_value(key, value))
        .transpose()
}

/// Parse a string-valued key from the document.
fn parse_string(doc: &DocumentMut, key: &str) -> Result<Option<String>, ConfigError> {
    Ok(string_at(doc, key)?.map(str::to_owned))
}

/// Reject any key outside `known`, naming it (qualified by `prefix`).
pub(super) fn reject_unknown_keys<'a>(
    keys: impl IntoIterator<Item = &'a str>,
    known: &[&str],
    prefix: &str,
) -> Result<(), ConfigError> {
    for key in keys {
        if !known.contains(&key) {
            return Err(ConfigError::UnknownKey {
                key: format!("{prefix}{key}"),
            });
        }
    }
    Ok(())
}

/// Parse the `remote` key from the document, as a validated [`Remote`].
fn parse_remote(doc: &DocumentMut, key: &str) -> Result<Option<Remote>, ConfigError> {
    string_at(doc, key)?
        .map(|value| parse_remote_value(key, value))
        .transpose()
}

/// Parse a remote URL, reporting the key on failure.
pub(super) fn parse_remote_value(key: &str, value: &str) -> Result<Remote, ConfigError> {
    value
        .parse::<Remote>()
        .map_err(|err| ConfigError::InvalidRemote {
            key: key.to_owned(),
            value: err.url,
        })
}

/// Read a key that must be a string, if present.
pub(super) fn string_at<'a>(
    doc: &'a DocumentMut,
    key: &str,
) -> Result<Option<&'a str>, ConfigError> {
    match doc.as_table().get(key) {
        None => Ok(None),
        Some(item) => Ok(Some(item_string(item, key)?)),
    }
}

/// Borrow an item as a string, or report a type error for `key`.
pub(super) fn item_string<'a>(item: &'a Item, key: &str) -> Result<&'a str, ConfigError> {
    item.as_str().ok_or_else(|| ConfigError::WrongType {
        key: key.to_owned(),
        expected: "string",
    })
}

/// Parse a value into `T`, reporting the key on failure.
pub(super) fn parse_value<T: FromStr<Err = UnknownValue>>(
    key: &str,
    value: &str,
) -> Result<T, ConfigError> {
    value.parse::<T>().map_err(|err| ConfigError::InvalidValue {
        key: key.to_owned(),
        value: err.value,
        expected: err.expected,
    })
}

/// Parse a duration-valued key (whole seconds) from the document.
fn parse_seconds_key(doc: &DocumentMut, key: &str) -> Result<Option<Duration>, ConfigError> {
    let Some(item) = doc.as_table().get(key) else {
        return Ok(None);
    };
    let seconds = item.as_integer().ok_or_else(|| ConfigError::WrongType {
        key: key.to_owned(),
        expected: "positive integer",
    })?;
    positive_seconds(key, seconds).map(Some)
}

/// Parse a duration-valued `MOMO_*` variable (whole seconds).
pub(super) fn parse_seconds(key: &str, value: &str) -> Result<Duration, ConfigError> {
    value
        .trim()
        .parse::<i64>()
        .map_err(|_| ConfigError::InvalidSeconds {
            key: key.to_owned(),
            value: value.to_owned(),
        })
        .and_then(|seconds| positive_seconds(key, seconds))
}

/// Reject a non-positive number of seconds, reporting `key`.
fn positive_seconds(key: &str, seconds: i64) -> Result<Duration, ConfigError> {
    if seconds <= 0 {
        return Err(ConfigError::InvalidSeconds {
            key: key.to_owned(),
            value: seconds.to_string(),
        });
    }
    Ok(Duration::from_secs(seconds as u64))
}
