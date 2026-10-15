//! Shared TOML machinery for the TOML-backed manifest adapters.
//!
//! Cargo's `Cargo.toml` and Python's `pyproject.toml` both edit a version
//! scalar in place with `toml_edit`, and both must leave the file's comments,
//! key order, whitespace and newline style alone. The mechanics are identical —
//! only which key holds the version differs — so they live here and each adapter
//! owns nothing but its key resolution.
//!
//! The edit is format-preserving by construction: the document is parsed into
//! `toml_edit`'s concrete syntax tree, one scalar is replaced, and the tree is
//! rendered back. Comments, key order, and whitespace survive byte-for-byte —
//! only the version value's own representation is rewritten. The source's
//! newline style (LF or CRLF) and whether it ended in a newline are reapplied
//! after rendering, because `toml_edit` normalises both.

use std::fs;
use std::path::Path;

use crate::domain::manifest::ManifestError;
use toml_edit::{DocumentMut, Item, Value};

/// Read `manifest`, attributing a failure to its path.
pub fn read(manifest: &Path) -> Result<String, ManifestError> {
    fs::read_to_string(manifest).map_err(|source| ManifestError::Read {
        path: manifest.to_path_buf(),
        source,
    })
}

/// Parse `text` as TOML, attributing a failure to `path`.
pub fn parse(text: &str, path: &Path) -> Result<DocumentMut, ManifestError> {
    text.parse().map_err(|source| ManifestError::Parse {
        path: path.to_path_buf(),
        source: Box::new(source),
    })
}

/// Replace `item`'s value with the string `value`, keeping its decor.
///
/// Replacing the value drops its decor — the whitespace around it and any
/// trailing comment — so the old decor is put back around the new scalar.
/// Returns whether the document changed: `false` when `item` already holds
/// `value`, or is not a scalar to replace, so a caller can tell an edit from a
/// no-op.
pub fn set_string(item: &mut Item, value: &str) -> bool {
    match item.as_value_mut() {
        Some(current) => set_value(current, value),
        None => false,
    }
}

/// Replace a scalar `value` with the string `text`, keeping its decor.
///
/// The [`Value`] counterpart of [`set_string`], for editing an array element
/// directly. Returns whether the value changed.
pub fn set_value(value: &mut Value, text: &str) -> bool {
    if value.as_str() == Some(text) {
        return false;
    }
    let decor = value.decor().clone();
    *value = Value::from(text);
    *value.decor_mut() = decor;
    true
}

/// Reapply `source`'s newline style and final-newline state to `rendered`.
///
/// `toml_edit` always renders LF and always terminates the document, so a CRLF
/// manifest or one without a trailing newline would otherwise be rewritten
/// wholesale. `rendered` is the untouched render of the parsed document.
pub fn preserve_line_endings(source: &str, rendered: String) -> String {
    let mut rendered = rendered;
    if !source.ends_with('\n') && rendered.ends_with('\n') {
        rendered.pop();
    }
    if uses_crlf(source) {
        rendered = rendered.replace('\n', "\r\n");
    }
    rendered
}

/// Whether `source` uses CRLF line endings throughout.
///
/// A file with any bare `\n`, or no `\r\n` at all, is treated as LF.
pub fn uses_crlf(source: &str) -> bool {
    source.contains("\r\n") && !source.replace("\r\n", "").contains('\n')
}
