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

mod env;
mod keys;
mod paths;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use toml_edit::{DocumentMut, Item, Table, value};

use crate::domain::remote::Remote;
use crate::domain::version::RcFormatError;

pub use paths::PackageConfig;

/// The default bound on any command momo spawns.
///
/// A hung child — jj workspace-lock contention is the realistic case — must fail
/// rather than stall an unattended release job forever.
pub const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

/// The header comment written above a generated `momo.toml`.
const HEADER: &str = "\
# momo configuration.
# Precedence: MOMO_* environment variables > this file > built-in defaults.
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
    /// How long any command momo spawns may run before it is killed.
    pub command_timeout: Duration,
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
            command_timeout: DEFAULT_COMMAND_TIMEOUT,
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
        if let Some(command_timeout) = overrides.command_timeout {
            self.command_timeout = command_timeout;
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
        if self.command_timeout != DEFAULT_COMMAND_TIMEOUT {
            table["command_timeout"] = value(self.command_timeout.as_secs() as i64);
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
    /// Override for [`Config::command_timeout`].
    pub command_timeout: Option<Duration>,
    /// Overrides for [`Config::packages`].
    pub packages: BTreeMap<String, PackageConfig>,
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
    /// A duration-valued key is not a positive whole number of seconds.
    #[error("`{key}` must be a positive whole number of seconds, got `{value}`")]
    InvalidSeconds {
        /// The offending key.
        key: String,
        /// The value that could not be used.
        value: String,
    },
}

impl From<toml_edit::TomlError> for ConfigError {
    fn from(err: toml_edit::TomlError) -> Self {
        Self::Toml(err.to_string())
    }
}
