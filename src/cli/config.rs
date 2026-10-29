//! Reading the effective configuration for the project root.
//!
//! The core crate owns the config model and the precedence rules; this module
//! supplies the two I/O layers - the `momo.toml` file and the `MOMO_*`
//! environment - and resolves them. Reading never creates the file.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::adapters::taskenv;
use crate::cli::atomic;
use crate::domain::config::{Config, ConfigOverrides};
use anyhow::{Context, Result};

/// The configuration file name at the project root.
pub const CONFIG_FILE: &str = "momo.toml";

/// Locate the project root by walking up from `start`.
///
/// The root is the nearest ancestor holding a VCS marker (`.jj` or `.git`) or a
/// `momo.toml`. With none of those, it is the nearest ancestor holding the mise
/// configuration — the same one detection finds — so a mise monorepo with no
/// VCS marker still roots at the directory mise reports its project roots
/// against. With neither, `start` itself is the root.
pub fn project_root(start: &Path) -> PathBuf {
    for dir in start.ancestors() {
        if dir.join(".jj").exists() || dir.join(".git").exists() || dir.join(CONFIG_FILE).exists() {
            return dir.to_path_buf();
        }
    }
    taskenv::config_root(start).unwrap_or_else(|| start.to_path_buf())
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

    let config = Config::resolve(file, env);
    config.validate().context("validating the configuration")?;
    Ok(config)
}

/// Write `contents` to the project's `momo.toml` under `root`, atomically.
///
/// Every `momo.toml` write goes through here so a crash or a full disk
/// mid-write cannot leave a torn TOML behind: [`atomic::write`] stages the whole
/// file beside the target and renames it into place, so a reader sees the whole
/// old file or the whole new one.
pub fn write(root: &Path, contents: &str) -> Result<()> {
    let path = root.join(CONFIG_FILE);
    atomic::write(&path, contents).with_context(|| format!("writing {}", path.display()))
}

/// Record the chosen task environment in the project's `momo.toml`.
///
/// `name` of `None` clears the key (used by `--uninstall`). The file is created
/// with the default configuration when it is missing and a name is given;
/// clearing never creates one. The write is skipped when the rendered file is
/// unchanged, so a repeat run leaves it byte-identical.
pub fn record_task_env(root: &Path, name: Option<&str>) -> Result<()> {
    let path = root.join(CONFIG_FILE);
    let existing = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err).with_context(|| format!("reading {}", path.display())),
    };

    let updated = if existing.trim().is_empty() {
        match name {
            Some(name) => Config {
                task_env: Some(name.to_owned()),
                ..Config::default()
            }
            .to_string(),
            None => return Ok(()),
        }
    } else {
        crate::domain::config::with_task_env(&existing, name)
            .with_context(|| format!("reading {}", path.display()))?
    };

    if updated == existing {
        return Ok(());
    }
    write(root, &updated)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn project_root_stops_at_the_mise_config_when_no_marker_is_found() {
        let root = TempDir::new().expect("temp dir");
        fs::write(root.path().join("mise.toml"), "monorepo_root = true\n")
            .expect("seed the mise config");
        let nested = root.path().join("crates/a");
        fs::create_dir_all(&nested).expect("create the nested package");

        assert_eq!(project_root(&nested), root.path());
    }

    #[test]
    fn project_root_finds_a_mise_config_under_conf_d() {
        let root = TempDir::new().expect("temp dir");
        fs::create_dir_all(root.path().join("mise/conf.d")).expect("create the conf.d directory");
        fs::write(
            root.path().join("mise/conf.d/monorepo.toml"),
            "monorepo_root = true\n",
        )
        .expect("seed the conf.d fragment");
        let nested = root.path().join("crates/a");
        fs::create_dir_all(&nested).expect("create the nested package");

        assert_eq!(project_root(&nested), root.path());
    }

    #[test]
    fn project_root_prefers_a_vcs_marker_over_a_nested_mise_config() {
        let root = TempDir::new().expect("temp dir");
        fs::create_dir(root.path().join(".git")).expect("create the git marker");
        let nested = root.path().join("crates/a");
        fs::create_dir_all(&nested).expect("create the nested package");
        fs::write(nested.join("mise.toml"), "monorepo_root = true\n")
            .expect("seed the mise config");

        assert_eq!(project_root(&nested), root.path());
    }

    #[test]
    fn project_root_prefers_a_nested_momo_toml_over_an_ancestor_mise_config() {
        let root = TempDir::new().expect("temp dir");
        fs::write(root.path().join("mise.toml"), "monorepo_root = true\n")
            .expect("seed the mise config");
        let nested = root.path().join("crates/a");
        fs::create_dir_all(&nested).expect("create the nested package");
        fs::write(nested.join(CONFIG_FILE), "").expect("seed the momo config");

        assert_eq!(project_root(&nested), nested);
    }

    #[test]
    fn project_root_falls_back_to_start_without_a_marker_or_mise_config() {
        let root = TempDir::new().expect("temp dir");
        let nested = root.path().join("crates/a");
        fs::create_dir_all(&nested).expect("create the nested directory");

        assert_eq!(project_root(&nested), nested);
    }

    #[test]
    fn write_replaces_the_config_atomically_and_leaves_no_temporary_behind() {
        let root = TempDir::new().expect("temp dir");
        fs::write(root.path().join(CONFIG_FILE), "old\n").expect("seed the config");

        write(root.path(), "new\n").expect("write the config");

        assert_eq!(
            fs::read_to_string(root.path().join(CONFIG_FILE)).expect("read the config"),
            "new\n"
        );
        let names: Vec<String> = fs::read_dir(root.path())
            .expect("read the root")
            .map(|entry| {
                entry
                    .expect("a directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(names, [CONFIG_FILE]);
    }
}
