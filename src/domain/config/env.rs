//! The `MOMO_*` environment layer of the configuration.

use std::path::PathBuf;

use super::ConfigError;
use super::ConfigOverrides;
use super::Recording;
use super::keys::{parse_remote_value, parse_seconds, parse_value};
use super::paths::validate_path;

/// The `MOMO_*` variables the configuration reads, in the order handled.
///
/// The environment surface is frozen and pinned by name in the tests below, for
/// the same reason the `momo.toml` keys are: see `docs/option-philosophy.md`.
/// A name outside this list is ignored, so an unrelated `MOMO_*` variable never
/// breaks a run.
const KNOWN_ENV_KEYS: [&str; 8] = [
    "MOMO_RECORDING",
    "MOMO_CHANGELOG",
    "MOMO_CHANGES_DIR",
    "MOMO_TAG_PREFIX",
    "MOMO_REMOTE",
    "MOMO_TASK_ENV",
    "MOMO_RC_FORMAT",
    "MOMO_COMMAND_TIMEOUT",
];

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
            if !KNOWN_ENV_KEYS.contains(&name) {
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
                "MOMO_COMMAND_TIMEOUT" => {
                    overrides.command_timeout = Some(parse_seconds(name, value)?);
                }
                _ => {}
            }
        }
        Ok(overrides)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_env_surface_is_frozen() {
        assert_eq!(
            KNOWN_ENV_KEYS,
            [
                "MOMO_RECORDING",
                "MOMO_CHANGELOG",
                "MOMO_CHANGES_DIR",
                "MOMO_TAG_PREFIX",
                "MOMO_REMOTE",
                "MOMO_TASK_ENV",
                "MOMO_RC_FORMAT",
                "MOMO_COMMAND_TIMEOUT",
            ]
        );
    }

    #[test]
    fn every_known_env_var_is_honoured() {
        // A name listed as known with no matching arm would be silently ignored,
        // so each one must change at least one override.
        for (name, value) in [
            ("MOMO_RECORDING", "fragments"),
            ("MOMO_CHANGELOG", "docs/CHANGELOG.md"),
            ("MOMO_CHANGES_DIR", "changes"),
            ("MOMO_TAG_PREFIX", "rel-"),
            ("MOMO_REMOTE", "https://github.com/a/b"),
            ("MOMO_TASK_ENV", "mise"),
            ("MOMO_RC_FORMAT", "-rc.{n}"),
            ("MOMO_COMMAND_TIMEOUT", "5"),
        ] {
            let overrides = ConfigOverrides::from_env([(name, value)])
                .unwrap_or_else(|err| panic!("`{name}` is not honoured: {err}"));
            assert_ne!(
                overrides,
                ConfigOverrides::default(),
                "`{name}` is listed as known but was ignored"
            );
        }
    }
}
