//! Path-key validation and the per-package overrides.

use std::collections::BTreeMap;
use std::path::{Component, PathBuf};

use toml_edit::DocumentMut;

use super::ConfigError;
use super::keys::{KNOWN_PACKAGE_KEYS, item_string, reject_unknown_keys, string_at};

/// Per-package overrides, keyed by the package's ecosystem identity.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PackageConfig {
    /// Override the package's ecosystem identity.
    pub identity: Option<String>,
    /// Override the package's derived, ref-safe slug.
    pub slug: Option<String>,
    /// Override the manifest synced with the Release version, relative to the
    /// package's directory. Without one the ecosystem's conventional manifest
    /// is discovered.
    pub manifest: Option<PathBuf>,
}

impl PackageConfig {
    /// Layer `other` over `self`, taking only the fields `other` sets.
    pub(super) fn overlay(&mut self, other: Self) {
        if let Some(identity) = other.identity {
            self.identity = Some(identity);
        }
        if let Some(slug) = other.slug {
            self.slug = Some(slug);
        }
        if let Some(manifest) = other.manifest {
            self.manifest = Some(manifest);
        }
    }
}

/// Parse a path-valued key from the document.
pub(super) fn parse_path(doc: &DocumentMut, key: &str) -> Result<Option<PathBuf>, ConfigError> {
    string_at(doc, key)?
        .map(PathBuf::from)
        .map(|path| validate_path(key, path))
        .transpose()
}

/// Validate a configured path: non-empty, relative, and not escaping upward.
///
/// A path is joined onto the package or repo root, so an absolute path would
/// discard that base and `..` would write outside the tree; both are rejected
/// rather than honoured silently.
pub(super) fn validate_path(key: &str, path: PathBuf) -> Result<PathBuf, ConfigError> {
    if path.as_os_str().is_empty() {
        return Err(ConfigError::EmptyPath {
            key: key.to_owned(),
        });
    }
    if path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
        return Err(ConfigError::InvalidPath {
            key: key.to_owned(),
            value: path.to_string_lossy().into_owned(),
        });
    }
    Ok(path)
}

/// Parse the `[packages]` table, if present.
pub(super) fn parse_packages(
    doc: &DocumentMut,
) -> Result<BTreeMap<String, PackageConfig>, ConfigError> {
    let Some(item) = doc.as_table().get("packages") else {
        return Ok(BTreeMap::new());
    };
    let table = item.as_table().ok_or_else(|| ConfigError::WrongType {
        key: "packages".to_owned(),
        expected: "table",
    })?;

    table
        .iter()
        .map(|(identity, item)| {
            let package = item.as_table().ok_or_else(|| ConfigError::WrongType {
                key: format!("packages.{identity}"),
                expected: "table",
            })?;
            reject_unknown_keys(
                package.iter().map(|(key, _)| key),
                &KNOWN_PACKAGE_KEYS,
                &format!("packages.{identity}."),
            )?;
            let field = |name: &str| -> Result<Option<String>, ConfigError> {
                package
                    .get(name)
                    .map(|item| item_string(item, &format!("packages.{identity}.{name}")))
                    .transpose()
                    .map(|value| value.map(str::to_owned))
            };
            Ok((
                identity.to_owned(),
                PackageConfig {
                    identity: field("identity")?,
                    slug: field("slug")?,
                    manifest: field("manifest")?
                        .map(PathBuf::from)
                        .map(|path| validate_path(&format!("packages.{identity}.manifest"), path))
                        .transpose()?,
                },
            ))
        })
        .collect()
}
