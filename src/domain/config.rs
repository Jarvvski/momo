//! The `momo.toml` configuration model and its resolution across layers.
//!
//! Configuration is resolved with a fixed precedence, lowest to highest:
//! built-in defaults, the `momo.toml` file, then `MOMO_*` environment
//! variables. Each layer only overrides the keys it actually sets, so a sparse
//! file leaves every other key at its default.
//!
//! The module is pure: it parses and renders TOML text and merges layers. It
//! never touches the filesystem or the process environment - reading
//! `momo.toml` and the environment is the CLI's job.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, PathBuf};
use std::str::FromStr;

use toml_edit::{DocumentMut, Item, Table, value};

use crate::domain::remote::Remote;
use crate::domain::version::RcFormatError;

/// The header comment written above a generated `momo.toml`.
const HEADER: &str = "\
# momo configuration.
# Precedence: MOMO_* environment variables > this file > built-in defaults.
";

/// The top-level keys a `momo.toml` may set.
const KNOWN_KEYS: [&str; 8] = [
    "recording",
    "changelog",
    "changes_dir",
    "tag_prefix",
    "remote",
    "task_env",
    "rc_format",
    "packages",
];

/// The keys a `[packages.<id>]` table may set.
const KNOWN_PACKAGE_KEYS: [&str; 3] = ["identity", "slug", "manifest"];

/// The error produced when a value is not one of the accepted variants.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown value `{value}`; expected one of: {expected}")]
pub struct UnknownValue {
    /// The value that could not be parsed.
    pub value: String,
    /// The accepted variants, comma-separated.
    pub expected: &'static str,
}

/// How momo records a Change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Recording {
    /// Changes accumulate under `## Unreleased` in the changelog.
    #[default]
    Changelog,
    /// Each Change is its own file under the changes directory.
    Fragments,
}

impl FromStr for Recording {
    type Err = UnknownValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "changelog" => Ok(Self::Changelog),
            "fragments" => Ok(Self::Fragments),
            other => Err(UnknownValue {
                value: other.to_owned(),
                expected: "changelog, fragments",
            }),
        }
    }
}

impl fmt::Display for Recording {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Changelog => "changelog",
            Self::Fragments => "fragments",
        })
    }
}

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
    fn overlay(&mut self, other: Self) {
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

/// The effective momo configuration, with every key resolved to a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// How Changes are recorded.
    pub recording: Recording,
    /// Path to the changelog, relative to the package root.
    pub changelog: PathBuf,
    /// Directory holding Fragment files.
    pub changes_dir: PathBuf,
    /// Prefix prepended to tag versions.
    pub tag_prefix: String,
    /// The code-host remote the Changelog's reference links point at, when
    /// configured. It overrides the remote derived from the VCS.
    ///
    /// Parsed when the configuration is read, so a value that is set but is not
    /// a remote URL is rejected there rather than silently ignored here.
    pub remote: Option<Remote>,
    /// The task environment momo is installed into, when any.
    pub task_env: Option<String>,
    /// Template for a Release Candidate version suffix, when customised.
    pub rc_format: Option<String>,
    /// Per-package overrides keyed by ecosystem identity.
    pub packages: BTreeMap<String, PackageConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            recording: Recording::default(),
            changelog: PathBuf::from("CHANGELOG.md"),
            changes_dir: PathBuf::from(".momo/changes"),
            tag_prefix: "v".to_owned(),
            remote: None,
            task_env: None,
            rc_format: None,
            packages: BTreeMap::new(),
        }
    }
}

impl Config {
    /// Resolve the effective configuration from its layers.
    ///
    /// Precedence, lowest to highest: built-in defaults, `file`, then `env`.
    pub fn resolve(file: ConfigOverrides, env: ConfigOverrides) -> Self {
        let mut config = Self::default();
        config.overlay(file);
        config.overlay(env);
        config
    }

    /// Layer `overrides` over `self`, taking only the keys it sets.
    fn overlay(&mut self, overrides: ConfigOverrides) {
        if let Some(recording) = overrides.recording {
            self.recording = recording;
        }
        if let Some(changelog) = overrides.changelog {
            self.changelog = changelog;
        }
        if let Some(changes_dir) = overrides.changes_dir {
            self.changes_dir = changes_dir;
        }
        if let Some(tag_prefix) = overrides.tag_prefix {
            self.tag_prefix = tag_prefix;
        }
        if let Some(remote) = overrides.remote {
            self.remote = Some(remote);
        }
        if let Some(task_env) = overrides.task_env {
            self.task_env = Some(task_env);
        }
        if let Some(rc_format) = overrides.rc_format {
            self.rc_format = Some(rc_format);
        }
        for (identity, package) in overrides.packages {
            self.packages.entry(identity).or_default().overlay(package);
        }
    }

    /// Validate the resolved configuration.
    ///
    /// The only key that can be internally inconsistent is `rc_format`: a
    /// template without exactly one `{n}`, or one that does not render to a
    /// valid pre-release, would mint the same candidate for every number. Called
    /// once the layers are merged, so both `momo.toml` and `MOMO_RC_FORMAT` are
    /// checked and the error names the offending value.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let Some(format) = &self.rc_format else {
            return Ok(());
        };
        crate::domain::version::validate_rc_format(format).map_err(|source| {
            ConfigError::InvalidRcFormat {
                format: format.clone(),
                source,
            }
        })
    }
}

impl fmt::Display for Config {
    /// Render the configuration as `momo.toml` text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut doc = DocumentMut::new();
        doc.as_table_mut().decor_mut().set_prefix(HEADER);

        let table = doc.as_table_mut();
        table["recording"] = value(self.recording.to_string());
        table["changelog"] = value(self.changelog.to_string_lossy().as_ref());
        table["changes_dir"] = value(self.changes_dir.to_string_lossy().as_ref());
        table["tag_prefix"] = value(self.tag_prefix.as_str());
        if let Some(remote) = &self.remote {
            table["remote"] = value(remote.base());
        }
        if let Some(task_env) = &self.task_env {
            table["task_env"] = value(task_env.as_str());
        }
        if let Some(rc_format) = &self.rc_format {
            table["rc_format"] = value(rc_format.as_str());
        }
        if !self.packages.is_empty() {
            let mut packages = Table::new();
            for (identity, package) in &self.packages {
                let mut entry = Table::new();
                if let Some(identity) = &package.identity {
                    entry["identity"] = value(identity.as_str());
                }
                if let Some(slug) = &package.slug {
                    entry["slug"] = value(slug.as_str());
                }
                if let Some(manifest) = &package.manifest {
                    entry["manifest"] = value(manifest.to_string_lossy().as_ref());
                }
                packages[identity.as_str()] = Item::Table(entry);
            }
            table["packages"] = Item::Table(packages);
        }

        f.write_str(&doc.to_string())
    }
}

/// A partial configuration: every key is optional so layers can be merged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConfigOverrides {
    /// Override for [`Config::recording`].
    pub recording: Option<Recording>,
    /// Override for [`Config::changelog`].
    pub changelog: Option<PathBuf>,
    /// Override for [`Config::changes_dir`].
    pub changes_dir: Option<PathBuf>,
    /// Override for [`Config::tag_prefix`].
    pub tag_prefix: Option<String>,
    /// Override for [`Config::remote`].
    pub remote: Option<Remote>,
    /// Override for [`Config::task_env`].
    pub task_env: Option<String>,
    /// Override for [`Config::rc_format`].
    pub rc_format: Option<String>,
    /// Overrides for [`Config::packages`].
    pub packages: BTreeMap<String, PackageConfig>,
}

impl ConfigOverrides {
    /// Parse overrides from `MOMO_*` environment entries.
    ///
    /// An empty value is treated as unset: an accidentally empty
    /// `MOMO_CHANGES_DIR=` must not redirect the Fragment store, and an empty
    /// `MOMO_TAG_PREFIX=` must not match every tag. Entries whose name is outside
    /// the known set are ignored, so unrelated `MOMO_*` variables never break a
    /// run.
    pub fn from_env<I, K, V>(vars: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut overrides = Self::default();
        for (name, raw) in vars {
            let name = name.as_ref();
            let value = raw.as_ref();
            if value.is_empty() {
                continue;
            }
            match name {
                "MOMO_RECORDING" => {
                    overrides.recording = Some(parse_value::<Recording>(name, value)?);
                }
                "MOMO_CHANGELOG" => {
                    overrides.changelog = Some(validate_path(name, PathBuf::from(value))?);
                }
                "MOMO_CHANGES_DIR" => {
                    overrides.changes_dir = Some(validate_path(name, PathBuf::from(value))?);
                }
                "MOMO_TAG_PREFIX" => overrides.tag_prefix = Some(value.to_owned()),
                "MOMO_REMOTE" => {
                    overrides.remote = Some(parse_remote_value(name, value)?);
                }
                "MOMO_TASK_ENV" => overrides.task_env = Some(value.to_owned()),
                "MOMO_RC_FORMAT" => overrides.rc_format = Some(value.to_owned()),
                _ => {}
            }
        }
        Ok(overrides)
    }
}

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
            packages: parse_packages(&doc)?,
        })
    }
}

/// Set the `task_env` key in `text`, or remove it when `name` is `None`.
///
/// The edit is format-preserving: every other key, comment, and blank line is
/// left exactly as written, so a hand-formatted `momo.toml` survives a
/// `momo install`.
pub fn with_task_env(text: &str, name: Option<&str>) -> Result<String, ConfigError> {
    let mut doc: DocumentMut = text.parse()?;
    match name {
        Some(name) => doc["task_env"] = value(name),
        None => {
            doc.as_table_mut().remove("task_env");
        }
    }
    Ok(doc.to_string())
}

/// Errors that can occur while reading configuration.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// The text is not valid TOML.
    #[error("invalid TOML: {0}")]
    Toml(String),
    /// A key has the wrong type.
    #[error("`{key}` must be a {expected}")]
    WrongType {
        /// The offending key.
        key: String,
        /// The type the key should have.
        expected: &'static str,
    },
    /// A key has an unrecognised value.
    #[error("invalid value `{value}` for `{key}`; expected one of: {expected}")]
    InvalidValue {
        /// The offending key.
        key: String,
        /// The value that could not be parsed.
        value: String,
        /// The accepted variants, comma-separated.
        expected: &'static str,
    },
    /// The document sets a key momo does not recognise.
    #[error("unknown key `{key}`")]
    UnknownKey {
        /// The unrecognised key, qualified by its table when nested.
        key: String,
    },
    /// A path-valued key is empty.
    #[error("`{key}` must not be empty")]
    EmptyPath {
        /// The offending key.
        key: String,
    },
    /// A path-valued key is absolute or escapes upward with `..`.
    #[error("`{key}` must be a relative path without `..`: `{value}`")]
    InvalidPath {
        /// The offending key.
        key: String,
        /// The value that could not be used.
        value: String,
    },
    /// The `remote` key is set to a value that is not a remote URL.
    #[error("invalid value `{value}` for `{key}`; expected a remote URL")]
    InvalidRemote {
        /// The offending key.
        key: String,
        /// The value that could not be parsed.
        value: String,
    },
    /// The `rc_format` is not a usable Release Candidate template.
    #[error("invalid value `{format}` for `rc_format`")]
    InvalidRcFormat {
        /// The offending value.
        format: String,
        /// Why the template is unusable.
        #[source]
        source: RcFormatError,
    },
}

impl From<toml_edit::TomlError> for ConfigError {
    fn from(err: toml_edit::TomlError) -> Self {
        Self::Toml(err.to_string())
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

/// Parse a path-valued key from the document.
fn parse_path(doc: &DocumentMut, key: &str) -> Result<Option<PathBuf>, ConfigError> {
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
fn validate_path(key: &str, path: PathBuf) -> Result<PathBuf, ConfigError> {
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

/// Reject any key outside `known`, naming it (qualified by `prefix`).
fn reject_unknown_keys<'a>(
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
fn parse_remote_value(key: &str, value: &str) -> Result<Remote, ConfigError> {
    value
        .parse::<Remote>()
        .map_err(|err| ConfigError::InvalidRemote {
            key: key.to_owned(),
            value: err.url,
        })
}

/// Read a key that must be a string, if present.
fn string_at<'a>(doc: &'a DocumentMut, key: &str) -> Result<Option<&'a str>, ConfigError> {
    match doc.as_table().get(key) {
        None => Ok(None),
        Some(item) => Ok(Some(item_string(item, key)?)),
    }
}

/// Borrow an item as a string, or report a type error for `key`.
fn item_string<'a>(item: &'a Item, key: &str) -> Result<&'a str, ConfigError> {
    item.as_str().ok_or_else(|| ConfigError::WrongType {
        key: key.to_owned(),
        expected: "string",
    })
}

/// Parse the `[packages]` table, if present.
fn parse_packages(doc: &DocumentMut) -> Result<BTreeMap<String, PackageConfig>, ConfigError> {
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

/// Parse a value into `T`, reporting the key on failure.
fn parse_value<T: FromStr<Err = UnknownValue>>(key: &str, value: &str) -> Result<T, ConfigError> {
    value.parse::<T>().map_err(|err| ConfigError::InvalidValue {
        key: key.to_owned(),
        value: err.value,
        expected: err.expected,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    #[test]
    fn defaults_match_the_spec() {
        let config = Config::default();
        assert_eq!(config.recording, Recording::Changelog);
        assert_eq!(config.changelog, PathBuf::from("CHANGELOG.md"));
        assert_eq!(config.changes_dir, PathBuf::from(".momo/changes"));
        assert_eq!(config.tag_prefix, "v");
        assert_eq!(config.remote, None);
        assert_eq!(config.task_env, None);
        assert_eq!(config.rc_format, None);
        assert!(config.packages.is_empty());
    }

    #[test]
    fn file_layer_overrides_defaults() {
        let file: ConfigOverrides = "recording = \"fragments\"\n".parse().expect("valid TOML");
        let config = Config::resolve(file, ConfigOverrides::default());
        assert_eq!(config.recording, Recording::Fragments);
        assert_eq!(config.changelog, PathBuf::from("CHANGELOG.md"));
    }

    #[test]
    fn env_layer_overrides_file() {
        let file: ConfigOverrides = "recording = \"fragments\"\n".parse().expect("valid TOML");
        let env =
            ConfigOverrides::from_env([("MOMO_RECORDING", "changelog")]).expect("valid env value");
        let config = Config::resolve(file, env);
        assert_eq!(config.recording, Recording::Changelog);
    }

    #[test]
    fn the_precedence_chain_is_monotonic() {
        let file: ConfigOverrides = "tag_prefix = \"file\"\n".parse().expect("valid TOML");
        let env = ConfigOverrides::from_env([("MOMO_TAG_PREFIX", "env")]).expect("valid env");
        assert_eq!(
            Config::resolve(file.clone(), ConfigOverrides::default()).tag_prefix,
            "file"
        );
        assert_eq!(Config::resolve(file, env).tag_prefix, "env");
    }

    #[test]
    fn a_remote_is_parsed_from_the_file() {
        let file: ConfigOverrides = "remote = \"git@github.com:a/b.git\"\n"
            .parse()
            .expect("valid TOML");
        let config = Config::resolve(file, ConfigOverrides::default());
        assert_eq!(
            config.remote.as_ref().map(Remote::base),
            Some("https://github.com/a/b")
        );
    }

    #[test]
    fn a_remote_is_parsed_from_the_environment() {
        let env = ConfigOverrides::from_env([("MOMO_REMOTE", "https://github.com/a/b")])
            .expect("valid env value");
        let config = Config::resolve(ConfigOverrides::default(), env);
        assert_eq!(
            config.remote.as_ref().map(Remote::base),
            Some("https://github.com/a/b")
        );
    }

    #[test]
    fn an_unparseable_remote_in_the_file_is_rejected() {
        let err = "remote = \"not a remote\"\n"
            .parse::<ConfigOverrides>()
            .expect_err("an unparseable remote fails");
        let message = err.to_string();
        assert!(message.contains("remote"), "{message}");
        assert!(message.contains("not a remote"), "{message}");
    }

    #[test]
    fn an_unparseable_remote_in_the_environment_is_rejected() {
        let err = ConfigOverrides::from_env([("MOMO_REMOTE", "not a remote")])
            .expect_err("an unparseable remote fails");
        let message = err.to_string();
        assert!(message.contains("MOMO_REMOTE"), "{message}");
        assert!(message.contains("not a remote"), "{message}");
    }

    #[test]
    fn an_rc_format_without_exactly_one_marker_is_rejected() {
        for format in ["-rc", "-rc.{n}.{n}"] {
            let config = Config {
                rc_format: Some(format.to_owned()),
                ..Config::default()
            };
            let err = config
                .validate()
                .expect_err("a template without exactly one `{n}` is rejected");
            let message = err.to_string();
            assert!(message.contains("rc_format"), "{message}");
            assert!(message.contains(format), "{message}");
        }
    }

    #[test]
    fn a_valid_rc_format_passes_validation() {
        let config = Config {
            rc_format: Some("-{n}.rc".to_owned()),
            ..Config::default()
        };
        assert!(config.validate().is_ok());
    }

    #[test]
    fn unknown_env_vars_are_ignored() {
        let env = ConfigOverrides::from_env([("MOMO_UNRELATED", "whatever")])
            .expect("unknown names are ignored");
        assert_eq!(env, ConfigOverrides::default());
    }

    #[test]
    fn unknown_env_value_is_rejected() {
        let err = ConfigOverrides::from_env([("MOMO_RECORDING", "nonsense")])
            .expect_err("unknown recording value fails");
        assert!(err.to_string().contains("MOMO_RECORDING"));
    }

    #[test]
    fn an_empty_env_value_is_treated_as_unset() {
        let env = ConfigOverrides::from_env([
            ("MOMO_TAG_PREFIX", "rel-"),
            ("MOMO_CHANGELOG", ""),
            ("MOMO_CHANGES_DIR", ""),
            ("MOMO_TASK_ENV", ""),
        ])
        .expect("empty values are ignored");
        assert_eq!(env.tag_prefix.as_deref(), Some("rel-"));
        assert_eq!(env.changelog, None);
        assert_eq!(env.changes_dir, None);
        assert_eq!(env.task_env, None);
    }

    #[test]
    fn an_absolute_path_in_the_environment_is_rejected() {
        let err = ConfigOverrides::from_env([("MOMO_CHANGELOG", "/tmp/x.md")])
            .expect_err("an absolute path fails");
        let message = err.to_string();
        assert!(message.contains("MOMO_CHANGELOG"), "{message}");
        assert!(message.contains("/tmp/x.md"), "{message}");
    }

    #[test]
    fn packages_are_parsed_and_layered() {
        let file: ConfigOverrides = "\
[packages.\"cargo:momo\"]
slug = \"cargo-momo\"
"
        .parse()
        .expect("valid TOML");
        let config = Config::resolve(file, ConfigOverrides::default());
        let package = config.packages.get("cargo:momo").expect("package present");
        assert_eq!(package.slug.as_deref(), Some("cargo-momo"));
        assert_eq!(package.identity, None);
        assert_eq!(package.manifest, None);
    }

    #[test]
    fn a_package_manifest_override_is_parsed() {
        let file: ConfigOverrides = "\
[packages.\"cargo:momo\"]
manifest = \"crates/momo/Cargo.toml\"
"
        .parse()
        .expect("valid TOML");
        let config = Config::resolve(file, ConfigOverrides::default());
        let package = config.packages.get("cargo:momo").expect("package present");
        assert_eq!(
            package.manifest.as_deref(),
            Some(Path::new("crates/momo/Cargo.toml"))
        );
    }

    #[test]
    fn generated_config_round_trips() {
        let mut original = Config {
            remote: Some(
                "https://github.com/Jarvvski/momo"
                    .parse()
                    .expect("a remote URL"),
            ),
            task_env: Some("mise".to_owned()),
            rc_format: Some("-rc.{n}".to_owned()),
            ..Config::default()
        };
        original.packages.insert(
            "cargo:momo".to_owned(),
            PackageConfig {
                identity: None,
                slug: Some("cargo-momo".to_owned()),
                manifest: Some(PathBuf::from("crates/momo/Cargo.toml")),
            },
        );

        let rendered = original.to_string();
        let parsed: ConfigOverrides = rendered.parse().expect("generated config is valid TOML");
        let resolved = Config::resolve(parsed, ConfigOverrides::default());
        assert_eq!(resolved, original);
    }

    #[test]
    fn wrong_type_is_rejected() {
        let err = "recording = 3\n"
            .parse::<ConfigOverrides>()
            .expect_err("a number is not a string");
        assert!(err.to_string().contains("recording"));
    }

    #[test]
    fn invalid_toml_is_rejected() {
        let err = "recording = \n"
            .parse::<ConfigOverrides>()
            .expect_err("invalid TOML fails");
        assert!(err.to_string().contains("invalid TOML"));
    }

    #[test]
    fn an_unknown_top_level_key_is_rejected() {
        let err = "recordingg = \"fragments\"\n"
            .parse::<ConfigOverrides>()
            .expect_err("an unknown key fails");
        assert!(err.to_string().contains("recordingg"), "{err}");
    }

    #[test]
    fn an_empty_path_in_the_file_is_rejected() {
        let err = "changelog = \"\"\n"
            .parse::<ConfigOverrides>()
            .expect_err("an empty path fails");
        assert!(err.to_string().contains("changelog"), "{err}");
    }

    #[test]
    fn an_absolute_path_in_the_file_is_rejected() {
        let err = "changes_dir = \"/tmp/x\"\n"
            .parse::<ConfigOverrides>()
            .expect_err("an absolute path fails");
        let message = err.to_string();
        assert!(message.contains("changes_dir"), "{message}");
        assert!(message.contains("/tmp/x"), "{message}");
    }

    #[test]
    fn a_parent_escaping_path_in_the_file_is_rejected() {
        let err = "changelog = \"../x.md\"\n"
            .parse::<ConfigOverrides>()
            .expect_err("a parent-escaping path fails");
        assert!(err.to_string().contains("changelog"), "{err}");
    }

    #[test]
    fn an_ordinary_relative_path_still_parses() {
        let file: ConfigOverrides = "changelog = \"docs/CHANGELOG.md\"\n"
            .parse()
            .expect("a relative path parses");
        assert_eq!(
            file.changelog.as_deref(),
            Some(Path::new("docs/CHANGELOG.md"))
        );
    }

    #[test]
    fn an_unknown_package_key_is_rejected() {
        let err = "[packages.\"cargo:momo\"]\nslgu = \"x\"\n"
            .parse::<ConfigOverrides>()
            .expect_err("an unknown package key fails");
        assert!(
            err.to_string().contains("packages.cargo:momo.slgu"),
            "{err}"
        );
    }

    #[test]
    fn a_parent_escaping_manifest_is_rejected() {
        let err = "[packages.\"cargo:momo\"]\nmanifest = \"a/../b\"\n"
            .parse::<ConfigOverrides>()
            .expect_err("a parent-escaping manifest fails");
        assert!(err.to_string().contains("manifest"), "{err}");
    }

    #[test]
    fn with_task_env_sets_the_key_and_preserves_the_rest() {
        let original = "# my config\nrecording = \"fragments\" # keep me\n";
        let updated = with_task_env(original, Some("mise")).expect("valid TOML");
        assert!(updated.contains("# my config"), "{updated}");
        assert!(
            updated.contains("recording = \"fragments\" # keep me"),
            "{updated}"
        );
        assert!(updated.contains("task_env = \"mise\""), "{updated}");
    }

    #[test]
    fn with_task_env_none_removes_the_key() {
        let original = "task_env = \"mise\"\nrecording = \"changelog\"\n";
        let updated = with_task_env(original, None).expect("valid TOML");
        assert!(!updated.contains("task_env"), "{updated}");
        assert!(updated.contains("recording = \"changelog\""), "{updated}");
    }

    #[test]
    fn with_task_env_is_idempotent() {
        let once = with_task_env("recording = \"changelog\"\n", Some("mise")).expect("valid TOML");
        let twice = with_task_env(&once, Some("mise")).expect("valid TOML");
        assert_eq!(once, twice);
    }
}
