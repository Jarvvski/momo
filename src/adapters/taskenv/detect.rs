//! mise detection: the nearest ancestor holding a mise config file or `.mise/`.
//!
//! Detection mirrors `momo-vcs`'s: the nearest ancestor wins, so a command run
//! from a subdirectory still finds the project's environment.

use std::path::{Path, PathBuf};

use crate::domain::taskenv::{TaskEnvError, TaskEnvironment};

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

/// The nearest ancestor of `start` holding a mise config file or `.mise` dir.
///
/// The names are [`CONFIG_FILES`], the same set mise reads, so a repo
/// configured with `.mise.toml` or `mise/config.toml` is detected too.
fn find_root(start: &Path) -> Option<PathBuf> {
    let absolute = std::path::absolute(start).ok()?;
    absolute
        .ancestors()
        .find(|dir| {
            CONFIG_FILES.iter().any(|name| dir.join(name).is_file()) || dir.join(".mise").is_dir()
        })
        .map(Path::to_path_buf)
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
