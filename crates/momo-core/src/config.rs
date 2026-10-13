//! The `momo.toml` configuration model and its resolution across layers.
//!
//! Configuration is resolved with a fixed precedence, lowest to highest:
//! built-in defaults, the `momo.toml` file, `MOMO_*` environment variables,
//! then CLI flags. Each layer only overrides the keys it actually sets, so a
//! sparse file leaves every other key at its default.
//!
//! The module is pure: it parses and renders TOML text and merges layers. It
//! never touches the filesystem or the process environment - reading
//! `momo.toml` and the environment is the CLI's job.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use toml_edit::{DocumentMut, Item, Table, value};

/// The header comment written above a generated `momo.toml`.
const HEADER: &str = "\
# momo configuration.
# Precedence: CLI flags > MOMO_* environment variables > this file > built-in defaults.
";

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

/// The kind of VCS tag momo creates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagKind {
    /// Defer to the VCS: annotated on git, lightweight on jj.
    #[default]
    Auto,
    /// Force a lightweight tag.
    Lightweight,
    /// Force an annotated tag.
    Annotated,
}

impl FromStr for TagKind {
    type Err = UnknownValue;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "lightweight" => Ok(Self::Lightweight),
            "annotated" => Ok(Self::Annotated),
            other => Err(UnknownValue {
                value: other.to_owned(),
                expected: "auto, lightweight, annotated",
            }),
        }
    }
}

impl fmt::Display for TagKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Auto => "auto",
            Self::Lightweight => "lightweight",
            Self::Annotated => "annotated",
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
    /// The kind of VCS tag to create.
    pub tag_kind: TagKind,
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
            tag_kind: TagKind::default(),
            task_env: None,
            rc_format: None,
            packages: BTreeMap::new(),
        }
    }
}

impl Config {
    /// Resolve the effective configuration from its layers.
    ///
    /// Precedence, lowest to highest: built-in defaults, `file`, `env`, then
    /// `flags`.
    pub fn resolve(file: ConfigOverrides, env: ConfigOverrides, flags: ConfigOverrides) -> Self {
        let mut config = Self::default();
        config.overlay(file);
        config.overlay(env);
        config.overlay(flags);
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
        if let Some(tag_kind) = overrides.tag_kind {
            self.tag_kind = tag_kind;
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
        table["tag_kind"] = value(self.tag_kind.to_string());
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
    /// Override for [`Config::tag_kind`].
    pub tag_kind: Option<TagKind>,
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
    /// Entries whose name is outside the known set are ignored, so unrelated
    /// `MOMO_*` variables never break a run.
    pub fn from_env<I, K, V>(vars: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let mut overrides = Self::default();
        for (name, raw) in vars {
            let value = raw.as_ref();
            match name.as_ref() {
                "MOMO_RECORDING" => {
                    overrides.recording = Some(parse_value::<Recording>(name.as_ref(), value)?);
                }
                "MOMO_CHANGELOG" => overrides.changelog = Some(PathBuf::from(value)),
                "MOMO_CHANGES_DIR" => overrides.changes_dir = Some(PathBuf::from(value)),
                "MOMO_TAG_PREFIX" => overrides.tag_prefix = Some(value.to_owned()),
                "MOMO_TAG_KIND" => {
                    overrides.tag_kind = Some(parse_value::<TagKind>(name.as_ref(), value)?);
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
        Ok(Self {
            recording: parse_scalar::<Recording>(&doc, "recording")?,
            changelog: parse_path(&doc, "changelog")?,
            changes_dir: parse_path(&doc, "changes_dir")?,
            tag_prefix: parse_string(&doc, "tag_prefix")?,
            tag_kind: parse_scalar::<TagKind>(&doc, "tag_kind")?,
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
    Ok(string_at(doc, key)?.map(PathBuf::from))
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
                    manifest: field("manifest")?.map(PathBuf::from),
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
        assert_eq!(config.tag_kind, TagKind::Auto);
        assert_eq!(config.task_env, None);
        assert_eq!(config.rc_format, None);
        assert!(config.packages.is_empty());
    }

    #[test]
    fn file_layer_overrides_defaults() {
        let file: ConfigOverrides = "recording = \"fragments\"\n".parse().expect("valid TOML");
        let config = Config::resolve(file, ConfigOverrides::default(), ConfigOverrides::default());
        assert_eq!(config.recording, Recording::Fragments);
        assert_eq!(config.changelog, PathBuf::from("CHANGELOG.md"));
    }

    #[test]
    fn env_layer_overrides_file() {
        let file: ConfigOverrides = "recording = \"fragments\"\n".parse().expect("valid TOML");
        let env =
            ConfigOverrides::from_env([("MOMO_RECORDING", "changelog")]).expect("valid env value");
        let config = Config::resolve(file, env, ConfigOverrides::default());
        assert_eq!(config.recording, Recording::Changelog);
    }

    #[test]
    fn flag_layer_overrides_env() {
        let file: ConfigOverrides = "recording = \"fragments\"\n".parse().expect("valid TOML");
        let env =
            ConfigOverrides::from_env([("MOMO_RECORDING", "changelog")]).expect("valid env value");
        let flags = ConfigOverrides {
            recording: Some(Recording::Fragments),
            ..ConfigOverrides::default()
        };
        let config = Config::resolve(file, env, flags);
        assert_eq!(config.recording, Recording::Fragments);
    }

    #[test]
    fn full_precedence_chain_is_monotonic() {
        let file: ConfigOverrides = "tag_prefix = \"file\"\n".parse().expect("valid TOML");
        let env = ConfigOverrides::from_env([("MOMO_TAG_PREFIX", "env")]).expect("valid env");
        let flags = ConfigOverrides {
            tag_prefix: Some("flag".to_owned()),
            ..ConfigOverrides::default()
        };
        assert_eq!(
            Config::resolve(
                file.clone(),
                ConfigOverrides::default(),
                ConfigOverrides::default()
            )
            .tag_prefix,
            "file"
        );
        assert_eq!(
            Config::resolve(file.clone(), env.clone(), ConfigOverrides::default()).tag_prefix,
            "env"
        );
        assert_eq!(Config::resolve(file, env, flags).tag_prefix, "flag");
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
    fn packages_are_parsed_and_layered() {
        let file: ConfigOverrides = "\
[packages.\"cargo:momo\"]
slug = \"cargo-momo\"
"
        .parse()
        .expect("valid TOML");
        let config = Config::resolve(file, ConfigOverrides::default(), ConfigOverrides::default());
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
        let config = Config::resolve(file, ConfigOverrides::default(), ConfigOverrides::default());
        let package = config.packages.get("cargo:momo").expect("package present");
        assert_eq!(
            package.manifest.as_deref(),
            Some(Path::new("crates/momo/Cargo.toml"))
        );
    }

    #[test]
    fn generated_config_round_trips() {
        let mut original = Config {
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
        let resolved = Config::resolve(
            parsed,
            ConfigOverrides::default(),
            ConfigOverrides::default(),
        );
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
