//! The `MOMO_*` environment layer of the configuration.

use std::path::PathBuf;

use super::ConfigError;
use super::ConfigOverrides;
use super::Recording;
use super::keys::{parse_remote_value, parse_seconds, parse_value};
use super::paths::validate_path;

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
                "MOMO_COMMAND_TIMEOUT" => {
                    overrides.command_timeout = Some(parse_seconds(name, value)?);
                }
                _ => {}
            }
        }
        Ok(overrides)
    }
}
