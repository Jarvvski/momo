//! mise detection: the nearest ancestor holding a mise config file, a
//! `conf.d` fragment or `.mise/`.
//!
//! Detection mirrors `momo-vcs`'s: the nearest ancestor wins, so a command run
//! from a subdirectory still finds the project's environment.

use std::path::{Path, PathBuf};

use crate::domain::taskenv::{TaskEnvError, TaskEnvironment};

use super::CONF_D_DIRS;
use super::CONFIG_FILES;
use super::mise::MiseTaskEnvironment;

/// Detect the mise project at or above `start`, if any.
pub fn detect(start: &Path) -> Option<Box<dyn TaskEnvironment>> {
    find_root(start)
        .map(|root| Box::new(MiseTaskEnvironment::new(root)) as Box<dyn TaskEnvironment>)
}

/// Build the named task environment rooted at `root`.
///
/// This backs the explicit `momo install <env>` override: it never consults the
/// filesystem, so an explicit environment installs even where none is detected.
pub fn named(name: &str, root: &Path) -> Result<Box<dyn TaskEnvironment>, TaskEnvError> {
    match name {
        "mise" => Ok(Box::new(MiseTaskEnvironment::new(root))),
        other => Err(TaskEnvError::Unknown {
            name: other.to_owned(),
        }),
    }
}

/// The nearest ancestor of `start` holding a mise config file, a `conf.d`
/// fragment or a `.mise` directory.
///
/// The names are [`CONFIG_FILES`] and [`CONF_D_DIRS`], the same set mise reads,
/// so a repo configured with `.mise.toml`, `mise/config.toml` or a
/// `mise/conf.d/*.toml` fragment is detected too. This is the same predicate
/// [`super::is_monorepo`] reads configuration with, so detection and the
/// monorepo check agree.
///
/// Re-exported as [`super::config_root`]: the CLI resolves the project root
/// with it when no VCS or `momo.toml` marker is found, so a mise monorepo with
/// neither still roots at the directory holding its configuration.
pub(crate) fn find_root(start: &Path) -> Option<PathBuf> {
    let absolute = std::path::absolute(start).ok()?;
    absolute
        .ancestors()
        .find(|dir| is_configured(dir))
        .map(Path::to_path_buf)
}

/// Whether `dir` directly holds mise configuration mise would read: a
/// [`CONFIG_FILES`] name, a `.mise` directory, or a `*.toml` fragment in a
/// [`CONF_D_DIRS`] directory.
fn is_configured(dir: &Path) -> bool {
    CONFIG_FILES.iter().any(|name| dir.join(name).is_file())
        || dir.join(".mise").is_dir()
        || CONF_D_DIRS
            .iter()
            .any(|name| holds_a_toml_fragment(&dir.join(name)))
}

/// Whether the directory `dir` holds at least one `*.toml` fragment.
///
/// A missing or unreadable directory holds no fragment.
fn holds_a_toml_fragment(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "toml")
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn detects_a_mise_toml_from_a_subdirectory() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("mise.toml"), "").unwrap();
        let nested = dir.path().join("crates/deep");
        std::fs::create_dir_all(&nested).unwrap();

        let environment = detect(&nested).expect("mise detected");
        assert_eq!(environment.root(), dir.path());
        assert_eq!(environment.name(), "mise");
    }

    #[test]
    fn detects_a_dot_mise_directory() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".mise")).unwrap();
        assert!(detect(dir.path()).is_some());
    }

    #[test]
    fn detects_a_dot_mise_toml_file() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join(".mise.toml"), "").unwrap();
        assert!(detect(dir.path()).is_some());
    }

    #[test]
    fn detects_a_conf_d_fragment() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("mise/conf.d")).unwrap();
        std::fs::write(
            dir.path().join("mise/conf.d/monorepo.toml"),
            "monorepo_root = true\n",
        )
        .unwrap();
        let nested = dir.path().join("crates/a");
        std::fs::create_dir_all(&nested).unwrap();

        let environment = detect(&nested).expect("mise detected");
        assert_eq!(environment.root(), dir.path());
        assert_eq!(environment.name(), "mise");
    }

    #[test]
    fn a_conf_d_directory_without_a_fragment_is_not_detected() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("mise/conf.d")).unwrap();
        assert!(detect(dir.path()).is_none());
    }

    #[test]
    fn detects_a_mise_config_toml_without_a_dot_mise_directory() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join("mise")).unwrap();
        std::fs::write(dir.path().join("mise/config.toml"), "").unwrap();
        let nested = dir.path().join("crates/deep");
        std::fs::create_dir_all(&nested).unwrap();

        let environment = detect(&nested).expect("mise detected");
        assert_eq!(environment.root(), dir.path());
    }

    #[test]
    fn returns_none_without_mise() {
        let dir = TempDir::new().unwrap();
        assert!(detect(dir.path()).is_none());
    }

    #[test]
    fn named_roots_at_the_given_directory() {
        let dir = TempDir::new().unwrap();
        let environment = named("mise", dir.path()).expect("mise is known");
        assert_eq!(environment.root(), dir.path());
    }

    #[test]
    fn named_rejects_an_unknown_environment() {
        let dir = TempDir::new().unwrap();
        let err = named("nonsense", dir.path())
            .err()
            .expect("an unknown environment fails");
        assert!(matches!(err, TaskEnvError::Unknown { ref name } if name == "nonsense"));
    }
}
