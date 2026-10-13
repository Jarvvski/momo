//! Reading the effective configuration for the project root.
//!
//! The core crate owns the config model and the precedence rules; this module
//! supplies the two I/O layers - the `momo.toml` file and the `MOMO_*`
//! environment - and resolves them. Reading never creates the file.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use momo_core::config::{Config, ConfigOverrides};

/// The configuration file name at the project root.
pub const CONFIG_FILE: &str = "momo.toml";

/// Locate the project root by walking up from `start`.
///
/// The root is the nearest ancestor holding a VCS marker (`.jj` or `.git`) or a
/// `momo.toml`; with none, `start` itself is the root.
pub fn project_root(start: &Path) -> PathBuf {
    for dir in start.ancestors() {
        if dir.join(".jj").exists() || dir.join(".git").exists() || dir.join(CONFIG_FILE).exists() {
            return dir.to_path_buf();
        }
    }
    start.to_path_buf()
}

/// Read the effective configuration for the project rooted at `root`.
///
/// Reads `momo.toml` and the `MOMO_*` environment, then layers them over the
/// built-in defaults. Missing files are not an error and are never created.
pub fn load(root: &Path) -> Result<Config> {
    let path = root.join(CONFIG_FILE);
    let file = match fs::read_to_string(&path) {
        Ok(text) => text
            .parse::<ConfigOverrides>()
            .with_context(|| format!("reading {}", path.display()))?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => ConfigOverrides::default(),
        Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
    };

    let env =
        ConfigOverrides::from_env(env::vars_os().filter_map(|(name, value)| {
            Some((name.into_string().ok()?, value.into_string().ok()?))
        }))
        .context("reading MOMO_* environment variables")?;

    Ok(Config::resolve(file, env, ConfigOverrides::default()))
}
