//! The Package: the unit of versioning, and the layout of its files.
//!
//! A single-package repo is a monorepo with one Package. A Package discovered in
//! a monorepo carries the ecosystem identity the project graph gave it (e.g.
//! `cargo:momo`) and a derived, ref-safe slug (e.g. `cargo-momo`); the fallback
//! single Package of a repo without a project graph takes the directory name as
//! its identity.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::slug::slugify;

/// The momo-managed version file, relative to a Package's directory.
pub const VERSION_FILE: &str = ".momo/version";

/// A unit of versioning: one package in a repo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    identity: String,
    slug: String,
    root: PathBuf,
    monorepo: bool,
    manifest: Option<PathBuf>,
}

impl Package {
    /// Assemble a Package from its parts.
    fn build(
        identity: impl Into<String>,
        slug: impl Into<String>,
        root: impl Into<PathBuf>,
        monorepo: bool,
    ) -> Self {
        Self {
            identity: identity.into(),
            slug: slug.into(),
            root: root.into(),
            monorepo,
            manifest: None,
        }
    }

    /// The only Package of a single-package repo named `name`.
    ///
    /// The identity is the directory name and the slug is its path-safe form.
    pub fn single(name: &str) -> Self {
        Self::build(name, slugify(name), PathBuf::from("."), false)
    }

    /// A Package discovered in a monorepo from its ecosystem `identity` and
    /// repo-relative directory `root`.
    ///
    /// The slug is derived from the identity: `cargo:momo` becomes `cargo-momo`.
    pub fn from_identity(identity: impl Into<String>, root: impl Into<PathBuf>) -> Self {
        let identity = identity.into();
        let slug = slugify(&identity);
        Self::build(identity, slug, root, true)
    }

    /// Apply this Package's configured overrides, when the config names it.
    pub fn configured(mut self, config: &Config) -> Self {
        if let Some(overrides) = config.packages.get(&self.identity) {
            if let Some(identity) = &overrides.identity {
                self.identity = identity.clone();
            }
            if let Some(slug) = &overrides.slug {
                self.slug = slug.clone();
            }
            if let Some(manifest) = &overrides.manifest {
                self.manifest = Some(manifest.clone());
            }
        }
        self
    }

    /// The ecosystem identity, e.g. `cargo:momo`.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// The ref/path-safe slug, e.g. `cargo-momo`.
    pub fn slug(&self) -> &str {
        &self.slug
    }

    /// The Package's directory relative to the repo root (`.` for the root).
    pub fn relative_root(&self) -> &Path {
        &self.root
    }

    /// Whether the Package was discovered in a monorepo.
    ///
    /// A monorepo Package namespaces its tags and its Fragment store by slug; a
    /// single-package repo keeps the flat, unnamespaced layout.
    pub fn is_monorepo(&self) -> bool {
        self.monorepo
    }

    /// The Package's directory under `repo_root`.
    pub fn dir(&self, repo_root: &Path) -> PathBuf {
        if self.root.as_os_str().is_empty() || self.root == Path::new(".") {
            repo_root.to_path_buf()
        } else {
            repo_root.join(&self.root)
        }
    }

    /// The Package's Changelog path, `config.changelog` under its directory.
    pub fn changelog(&self, repo_root: &Path, config: &Config) -> PathBuf {
        self.dir(repo_root).join(&config.changelog)
    }

    /// The Package's Fragment store.
    ///
    /// Repo-level under `config.changes_dir`, namespaced by slug in a monorepo.
    pub fn changes_dir(&self, repo_root: &Path, config: &Config) -> PathBuf {
        let base = repo_root.join(&config.changes_dir);
        if self.monorepo {
            base.join(&self.slug)
        } else {
            base
        }
    }

    /// The Package's version-file path.
    pub fn version_file(&self, repo_root: &Path) -> PathBuf {
        self.dir(repo_root).join(VERSION_FILE)
    }

    /// The configured manifest path override, relative to the Package's
    /// directory.
    ///
    /// `None` when the Package's ecosystem manifest is left to auto-discovery.
    pub fn manifest_override(&self) -> Option<&Path> {
        self.manifest.as_deref()
    }

    /// The Package's tag prefix: `<slug>-<base>` in a monorepo, `<base>` alone
    /// for a single package.
    pub fn tag_prefix(&self, base: &str) -> String {
        if self.monorepo {
            format!("{}-{base}", self.slug)
        } else {
            base.to_owned()
        }
    }

    /// Whether `query` names this Package: its identity, slug, or bare name.
    ///
    /// The bare name is the identity after the provider namespace, so `momo`
    /// matches `cargo:momo` and `@acme/ui` matches `node:@acme/ui`.
    pub fn matches(&self, query: &str) -> bool {
        query == self.identity
            || query == self.slug
            || self
                .identity
                .split_once(':')
                .is_some_and(|(_, name)| name == query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::PathBuf;

    fn config() -> Config {
        Config::default()
    }

    #[test]
    fn a_single_package_derives_its_slug_from_its_name() {
        let package = Package::single("My Project");
        assert_eq!(package.identity(), "My Project");
        assert_eq!(package.slug(), "my-project");
        assert!(!package.is_monorepo());
    }

    #[test]
    fn a_discovered_package_derives_its_slug_from_the_ecosystem_id() {
        let package = Package::from_identity("cargo:momo", "crates/momo");
        assert_eq!(package.identity(), "cargo:momo");
        assert_eq!(package.slug(), "cargo-momo");
        assert_eq!(package.relative_root(), Path::new("crates/momo"));
        assert!(package.is_monorepo());

        let scoped = Package::from_identity("node:@acme/ui", "packages/ui");
        assert_eq!(scoped.slug(), "node-acme-ui");
    }

    #[test]
    fn a_single_package_lives_at_the_repo_root() {
        let root = Path::new("/repo");
        let package = Package::single("momo");
        assert_eq!(package.dir(root), PathBuf::from("/repo"));
        assert_eq!(
            package.changelog(root, &config()),
            root.join("CHANGELOG.md")
        );
        assert_eq!(
            package.changes_dir(root, &config()),
            root.join(".momo/changes")
        );
        assert_eq!(package.version_file(root), root.join(".momo/version"));
    }

    #[test]
    fn a_monorepo_package_namespaces_its_changelog_store_and_tags() {
        let root = Path::new("/repo");
        let package = Package::from_identity("cargo:momo", "crates/momo");

        assert_eq!(package.dir(root), PathBuf::from("/repo/crates/momo"));
        assert_eq!(
            package.changelog(root, &config()),
            root.join("crates/momo/CHANGELOG.md")
        );
        assert_eq!(
            package.changes_dir(root, &config()),
            root.join(".momo/changes/cargo-momo")
        );
        assert_eq!(
            package.version_file(root),
            root.join("crates/momo/.momo/version")
        );
    }

    #[test]
    fn a_monorepo_package_at_the_root_is_not_suffixed_with_a_dot() {
        let root = Path::new("/repo");
        let package = Package::from_identity("cargo:rootpkg", ".");
        assert_eq!(package.dir(root), PathBuf::from("/repo"));
        assert_eq!(
            package.changelog(root, &config()),
            root.join("CHANGELOG.md")
        );
    }

    #[test]
    fn the_tag_prefix_is_namespaced_only_in_a_monorepo() {
        assert_eq!(Package::single("momo").tag_prefix("v"), "v");
        assert_eq!(
            Package::from_identity("cargo:momo", "crates/momo").tag_prefix("v"),
            "cargo-momo-v"
        );
        assert_eq!(
            Package::from_identity("cargo:momo", "crates/momo").tag_prefix("rel-"),
            "cargo-momo-rel-"
        );
    }

    #[test]
    fn a_package_matches_its_identity_slug_or_bare_name() {
        let package = Package::from_identity("cargo:momo", "crates/momo");
        assert!(package.matches("cargo:momo"));
        assert!(package.matches("cargo-momo"));
        assert!(package.matches("momo"));
        assert!(!package.matches("other"));

        let scoped = Package::from_identity("node:@acme/ui", "packages/ui");
        assert!(scoped.matches("@acme/ui"));
        assert!(scoped.matches("node-acme-ui"));
    }

    #[test]
    fn configured_overrides_the_identity_and_slug() {
        let mut config = Config::default();
        config.packages.insert(
            "cargo:momo".to_owned(),
            crate::config::PackageConfig {
                identity: Some("cargo:renamed".to_owned()),
                slug: Some("alpha".to_owned()),
                manifest: None,
            },
        );
        let package = Package::from_identity("cargo:momo", "crates/momo").configured(&config);
        assert_eq!(package.identity(), "cargo:renamed");
        assert_eq!(package.slug(), "alpha");
    }

    #[test]
    fn configured_takes_the_manifest_override() {
        let mut config = Config::default();
        config.packages.insert(
            "cargo:momo".to_owned(),
            crate::config::PackageConfig {
                identity: None,
                slug: None,
                manifest: Some(PathBuf::from("manifest.toml")),
            },
        );
        let package = Package::from_identity("cargo:momo", "crates/momo").configured(&config);
        assert_eq!(
            package.manifest_override(),
            Some(Path::new("manifest.toml"))
        );
    }

    #[test]
    fn configured_leaves_an_unmentioned_package_alone() {
        let package = Package::from_identity("cargo:momo", "crates/momo").configured(&config());
        assert_eq!(package.identity(), "cargo:momo");
        assert_eq!(package.slug(), "cargo-momo");
        assert_eq!(package.manifest_override(), None);
    }
}
