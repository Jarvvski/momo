//! The Workspace: the Packages a repo holds, and how a command selects among
//! them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::domain::config::Config;
use crate::domain::graph::DependencyGraph;
use crate::domain::package::Package;

/// Why a Workspace could not be assembled.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    /// Two Packages derive the same slug, so they would share a tag prefix and
    /// a Fragment store.
    #[error(
        "packages {} share the slug `{slug}`; give one a distinct `slug` override",
        identities.join(", ")
    )]
    DuplicateSlug {
        /// The colliding slug.
        slug: String,
        /// The fully-qualified identities that collide, sorted.
        identities: Vec<String>,
    },
}

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

    /// More than one Package matches the requested name.
    #[error(
        "`{query}` matches more than one package: {}",
        candidates.join(", ")
    )]
    Ambiguous {
        /// The name that matched several Packages.
        query: String,
        /// The fully-qualified identities that matched.
        candidates: Vec<String>,
    },

    /// The current directory is not inside any Package.
    #[error("no package contains `{cwd}`; pass `-p <package>` or `--all`")]
    NotInPackage {
        /// The directory that matched nothing.
        cwd: PathBuf,
    },
}

/// The Packages a repo holds, and the dependency graph discovery produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    root: PathBuf,
    packages: Vec<Package>,
    graph: DependencyGraph,
}

impl Workspace {
    /// A Workspace of `packages` and their dependency `graph`, with the config's
    /// per-package overrides applied.
    ///
    /// Fails when two Packages derive the same slug: they would share a tag
    /// prefix and a Fragment store, so the collision is fatal rather than
    /// silently resolved. A `slug` override can disambiguate before the check.
    pub fn new(
        root: impl Into<PathBuf>,
        packages: Vec<Package>,
        graph: DependencyGraph,
        config: &Config,
    ) -> Result<Self, WorkspaceError> {
        let packages: Vec<Package> = packages
            .into_iter()
            .map(|package| package.configured(config))
            .collect();
        Self::check_unique_slugs(&packages)?;
        Ok(Self {
            root: root.into(),
            packages,
            graph,
        })
    }

    /// The single-Package Workspace used when discovery is unavailable.
    pub fn single(root: impl Into<PathBuf>, name: &str, config: &Config) -> Self {
        Self {
            root: root.into(),
            packages: vec![Package::single(name).configured(config)],
            graph: DependencyGraph::default(),
        }
    }

    /// The dependency graph discovery produced; empty for a single-package repo.
    pub fn graph(&self) -> &DependencyGraph {
        &self.graph
    }

    /// Reject Packages whose slugs collide.
    ///
    /// The check is on the slug alone: two identities that are distinct can
    /// still collapse to one slug (`node:@acme/ui` and `node:acme-ui`), and so
    /// can two that share an ecosystem (`cargo:a_b` and `cargo:a-b`).
    fn check_unique_slugs(packages: &[Package]) -> Result<(), WorkspaceError> {
        let mut by_slug: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for package in packages {
            by_slug
                .entry(package.slug())
                .or_default()
                .push(package.identity());
        }
        for (slug, identities) in by_slug {
            if identities.len() > 1 {
                let mut identities: Vec<String> =
                    identities.into_iter().map(str::to_owned).collect();
                identities.sort();
                return Err(WorkspaceError::DuplicateSlug {
                    slug: slug.to_owned(),
                    identities,
                });
            }
        }
        Ok(())
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
            let matches: Vec<&Package> = self
                .packages
                .iter()
                .filter(|entry| entry.matches(query))
                .collect();
            return match matches.as_slice() {
                [] => Err(SelectionError::Unknown {
                    query: query.to_owned(),
                }),
                [only] => Ok(vec![*only]),
                many => Err(SelectionError::Ambiguous {
                    query: query.to_owned(),
                    candidates: many
                        .iter()
                        .map(|package| package.identity().to_owned())
                        .collect(),
                }),
            };
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

    use crate::domain::config::PackageConfig;

    use crate::domain::graph::{Edge, Project};

    /// A workspace with two discovered Packages.
    fn monorepo() -> Workspace {
        Workspace::new(
            "/repo",
            vec![
                Package::from_identity("cargo:a", "crates/a"),
                Package::from_identity("cargo:b", "crates/b"),
            ],
            DependencyGraph::default(),
            &Config::default(),
        )
        .expect("distinct slugs")
    }

    #[test]
    fn the_workspace_exposes_the_discovered_graph() {
        let graph = DependencyGraph::new(
            vec![
                Project::new("cargo:a", "crates/a", None, None, None),
                Project::new("cargo:b", "crates/b", None, None, None),
            ],
            vec![Edge::new("cargo:b", "cargo:a", None)],
        );
        let workspace = Workspace::new(
            "/repo",
            vec![
                Package::from_identity("cargo:a", "crates/a"),
                Package::from_identity("cargo:b", "crates/b"),
            ],
            graph,
            &Config::default(),
        )
        .expect("distinct slugs");

        assert_eq!(workspace.graph().projects().len(), 2);
        assert_eq!(workspace.graph().dependents_of("cargo:a").len(), 1);
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
            DependencyGraph::default(),
            &config,
        )
        .expect("a single package cannot collide");
        assert_eq!(workspace.packages()[0].slug(), "alpha");
    }

    #[test]
    fn distinct_identities_that_collapse_to_one_slug_are_rejected() {
        let err = Workspace::new(
            "/repo",
            vec![
                Package::from_identity("node:@acme/ui", "packages/scoped"),
                Package::from_identity("node:acme-ui", "packages/plain"),
            ],
            DependencyGraph::default(),
            &Config::default(),
        )
        .expect_err("both slug to node-acme-ui");
        match err {
            WorkspaceError::DuplicateSlug { slug, identities } => {
                assert_eq!(slug, "node-acme-ui");
                assert_eq!(identities, vec!["node:@acme/ui", "node:acme-ui"]);
            }
        }
    }

    #[test]
    fn a_slug_override_resolves_a_collision() {
        let mut config = Config::default();
        config.packages.insert(
            "node:acme-ui".to_owned(),
            PackageConfig {
                identity: None,
                slug: Some("node-acme-ui-plain".to_owned()),
                manifest: None,
            },
        );
        let workspace = Workspace::new(
            "/repo",
            vec![
                Package::from_identity("node:@acme/ui", "packages/scoped"),
                Package::from_identity("node:acme-ui", "packages/plain"),
            ],
            DependencyGraph::default(),
            &config,
        )
        .expect("the override disambiguates the slug");
        assert_eq!(workspace.packages()[0].slug(), "node-acme-ui");
        assert_eq!(workspace.packages()[1].slug(), "node-acme-ui-plain");
    }

    #[test]
    fn a_bare_name_matching_several_packages_is_rejected() {
        let workspace = Workspace::new(
            "/repo",
            vec![
                Package::from_identity("cargo:ui", "crates/ui"),
                Package::from_identity("node:ui", "packages/ui"),
            ],
            DependencyGraph::default(),
            &Config::default(),
        )
        .expect("distinct slugs");
        let err = workspace
            .select(Some("ui"), false, DefaultTarget::All, Path::new("/repo"))
            .expect_err("the bare name is ambiguous");
        match err {
            SelectionError::Ambiguous { query, candidates } => {
                assert_eq!(query, "ui");
                assert_eq!(candidates, vec!["cargo:ui", "node:ui"]);
            }
            other => panic!("expected an ambiguous-selection error, got {other}"),
        }

        // A fully-qualified identity stays unambiguous.
        let selected = workspace
            .select(
                Some("cargo:ui"),
                false,
                DefaultTarget::All,
                Path::new("/repo"),
            )
            .expect("a qualified identity");
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].identity(), "cargo:ui");
    }
}
