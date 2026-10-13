//! The Workspace: the Packages a repo holds, and how a command selects among
//! them.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::package::Package;

/// What a command targets when neither `--package` nor `--all` is given.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultTarget {
    /// Every Package — the aggregate verbs (`status`, `release`, `tag`).
    All,
    /// The Package containing the current directory — the recording verbs.
    Cwd,
}

/// Why a target selection failed.
#[derive(Debug, thiserror::Error)]
pub enum SelectionError {
    /// No Package matches the requested name.
    #[error("no package matches `{query}`")]
    Unknown {
        /// The name that matched nothing.
        query: String,
    },

    /// `--all` and `--package` were both given.
    #[error("`--all` and `--package` cannot be combined")]
    Conflicting,

    /// The current directory is not inside any Package.
    #[error("no package contains `{cwd}`; pass `-p <package>` or `--all`")]
    NotInPackage {
        /// The directory that matched nothing.
        cwd: PathBuf,
    },
}

/// The Packages a repo holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    root: PathBuf,
    packages: Vec<Package>,
}

impl Workspace {
    /// A Workspace of `packages`, with the config's per-package overrides
    /// applied.
    pub fn new(root: impl Into<PathBuf>, packages: Vec<Package>, config: &Config) -> Self {
        Self {
            root: root.into(),
            packages: packages
                .into_iter()
                .map(|package| package.configured(config))
                .collect(),
        }
    }

    /// The single-Package Workspace used when discovery is unavailable.
    pub fn single(root: impl Into<PathBuf>, name: &str, config: &Config) -> Self {
        Self::new(root, vec![Package::single(name)], config)
    }

    /// The repo root every Package is relative to.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every Package, in discovery order.
    pub fn packages(&self) -> &[Package] {
        &self.packages
    }

    /// Select the Packages a command targets.
    ///
    /// `--all` takes every Package and `--package` takes the one named `query`;
    /// with neither, `default` decides. Selection by `query` accepts a Package's
    /// identity, slug, or bare name.
    pub fn select(
        &self,
        package: Option<&str>,
        all: bool,
        default: DefaultTarget,
        cwd: &Path,
    ) -> Result<Vec<&Package>, SelectionError> {
        if all && package.is_some() {
            return Err(SelectionError::Conflicting);
        }
        if all {
            return Ok(self.packages.iter().collect());
        }
        if let Some(query) = package {
            let found = self.packages.iter().find(|entry| entry.matches(query));
            return found
                .map(|entry| vec![entry])
                .ok_or_else(|| SelectionError::Unknown {
                    query: query.to_owned(),
                });
        }
        match default {
            DefaultTarget::All => Ok(self.packages.iter().collect()),
            DefaultTarget::Cwd => self
                .packages
                .iter()
                .filter(|entry| cwd.starts_with(entry.dir(&self.root)))
                .max_by_key(|entry| entry.dir(&self.root).components().count())
                .map(|entry| vec![entry])
                .ok_or_else(|| SelectionError::NotInPackage {
                    cwd: cwd.to_path_buf(),
                }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::PackageConfig;

    /// A workspace with two discovered Packages.
    fn monorepo() -> Workspace {
        Workspace::new(
            "/repo",
            vec![
                Package::from_identity("cargo:a", "crates/a"),
                Package::from_identity("cargo:b", "crates/b"),
            ],
            &Config::default(),
        )
    }

    #[test]
    fn all_selects_every_package() {
        let workspace = monorepo();
        let selected = workspace
            .select(None, true, DefaultTarget::Cwd, Path::new("/repo"))
            .expect("all packages");
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn a_named_package_is_selected_by_identity_slug_or_bare_name() {
        let workspace = monorepo();
        for query in ["cargo:a", "cargo-a", "a"] {
            let selected = workspace
                .select(Some(query), false, DefaultTarget::All, Path::new("/repo"))
                .expect("a match");
            assert_eq!(selected.len(), 1);
            assert_eq!(selected[0].identity(), "cargo:a");
        }
    }

    #[test]
    fn an_unknown_name_is_rejected() {
        let workspace = monorepo();
        let err = workspace
            .select(Some("nope"), false, DefaultTarget::All, Path::new("/repo"))
            .expect_err("no match");
        assert!(matches!(err, SelectionError::Unknown { .. }));
    }

    #[test]
    fn combining_all_and_package_is_rejected() {
        let workspace = monorepo();
        let err = workspace
            .select(
                Some("cargo:a"),
                true,
                DefaultTarget::All,
                Path::new("/repo"),
            )
            .expect_err("conflicting flags");
        assert!(matches!(err, SelectionError::Conflicting));
    }

    #[test]
    fn the_default_all_takes_every_package() {
        let workspace = monorepo();
        let selected = workspace
            .select(None, false, DefaultTarget::All, Path::new("/repo"))
            .expect("all packages");
        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn the_default_cwd_takes_the_containing_package() {
        let workspace = monorepo();
        let selected = workspace
            .select(
                None,
                false,
                DefaultTarget::Cwd,
                Path::new("/repo/crates/b/src"),
            )
            .expect("the containing package");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].identity(), "cargo:b");
    }

    #[test]
    fn the_default_cwd_outside_every_package_is_rejected() {
        let workspace = monorepo();
        let err = workspace
            .select(None, false, DefaultTarget::Cwd, Path::new("/repo"))
            .expect_err("no containing package");
        assert!(matches!(err, SelectionError::NotInPackage { .. }));
    }

    #[test]
    fn a_single_package_workspace_contains_the_current_directory() {
        let workspace = Workspace::single("/repo", "momo", &Config::default());
        let selected = workspace
            .select(
                None,
                false,
                DefaultTarget::Cwd,
                Path::new("/repo/crates/deep"),
            )
            .expect("the single package");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].identity(), "momo");
    }

    #[test]
    fn per_package_overrides_apply_to_discovered_packages() {
        let mut config = Config::default();
        config.packages.insert(
            "cargo:a".to_owned(),
            PackageConfig {
                identity: None,
                slug: Some("alpha".to_owned()),
                manifest: None,
            },
        );
        let workspace = Workspace::new(
            "/repo",
            vec![Package::from_identity("cargo:a", "crates/a")],
            &config,
        );
        assert_eq!(workspace.packages()[0].slug(), "alpha");
    }
}
