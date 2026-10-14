//! Manifest adapters: the Rust `Cargo.toml` and Python `pyproject.toml`,
//! implementing a port from [`crate::domain::manifest`].

mod cargo;
mod python;
mod toml;

#[cfg(test)]
mod roundtrip;

pub use cargo::CargoManifest;
pub use python::PythonManifest;

use std::path::{Path, PathBuf};

use crate::domain::manifest::{ManifestError, ManifestPlan, ManifestSync, SkipReason};
use crate::domain::package::Package;
use semver::Version;

/// The [`ManifestSync`] adapter for `package`, chosen by its ecosystem.
///
/// The ecosystem is the Package identity's prefix — `cargo:momo` is a Cargo
/// package, `uv:acme` a Python one. A single-package repo carries no prefix, so
/// every ecosystem's conventional manifest is probed ([`Probe`]). `None` when
/// the Package's ecosystem has no adapter yet, so a Release skips manifest sync.
pub fn for_package(package: &Package) -> Option<Box<dyn ManifestSync>> {
    match provider(package.identity()) {
        Some("cargo") => Some(Box::new(CargoManifest)),
        Some("uv") => Some(Box::new(PythonManifest)),
        None => Some(Box::new(Probe::new())),
        Some(_) => None,
    }
}

/// The ecosystem prefix of an identity, e.g. `cargo` for `cargo:momo`.
fn provider(identity: &str) -> Option<&str> {
    identity.split_once(':').map(|(provider, _)| provider)
}

/// A single-package repo's adapter: every ecosystem's adapter, tried in order.
///
/// A single-package repo carries no ecosystem prefix, so the manifest on disk
/// decides — each adapter is asked for its conventional manifest and the first
/// one present wins (Cargo before Python, so a repo holding both syncs its
/// `Cargo.toml`). A configured `manifest` override names the file directly; the
/// adapter whose conventional manifest it is owns it, and failing that the first
/// adapter to accept it does.
struct Probe {
    /// The adapters probed, in order.
    adapters: Vec<Box<dyn ManifestSync>>,
}

impl Probe {
    /// The adapters probed, in order.
    fn new() -> Self {
        Self {
            adapters: vec![Box::new(CargoManifest), Box::new(PythonManifest)],
        }
    }

    /// The adapter that owns `manifest`, when one does.
    ///
    /// A conventionally discovered manifest is claimed by the adapter that
    /// discovers it; a configured override is claimed by whichever adapter
    /// accepts that path first.
    fn owner(&self, manifest: &Path) -> Option<&dyn ManifestSync> {
        let dir = manifest.parent()?;
        let name = manifest.file_name()?;
        self.adapters
            .iter()
            .find(|adapter| adapter.locate(dir, None).as_deref() == Some(manifest))
            .or_else(|| {
                self.adapters.iter().find(|adapter| {
                    adapter.locate(dir, Some(Path::new(name))).as_deref() == Some(manifest)
                })
            })
            .map(|adapter| adapter.as_ref())
    }
}

impl ManifestSync for Probe {
    fn locate(&self, package_dir: &Path, override_path: Option<&Path>) -> Option<PathBuf> {
        match override_path {
            Some(override_path) => {
                let path = package_dir.join(override_path);
                path.is_file().then_some(path)
            }
            None => self
                .adapters
                .iter()
                .find_map(|adapter| adapter.locate(package_dir, None)),
        }
    }

    fn render(&self, manifest: &Path, version: &Version) -> Result<ManifestPlan, ManifestError> {
        match self.owner(manifest) {
            Some(adapter) => adapter.render(manifest, version),
            None => Ok(unowned(manifest)),
        }
    }

    fn write(&self, manifest: &Path, version: &Version) -> Result<ManifestPlan, ManifestError> {
        match self.owner(manifest) {
            Some(adapter) => adapter.write(manifest, version),
            None => Ok(unowned(manifest)),
        }
    }
}

/// A plan that writes nothing: no adapter owns `manifest`.
fn unowned(manifest: &Path) -> ManifestPlan {
    ManifestPlan {
        writes: Vec::new(),
        lockfiles: Vec::new(),
        skipped: vec![(manifest.to_path_buf(), SkipReason::NoVersion)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    #[test]
    fn a_cargo_package_gets_the_cargo_adapter() {
        let package = Package::from_identity("cargo:momo", "crates/momo");
        let sync = for_package(&package).expect("cargo has an adapter");
        assert_eq!(sync.locate(std::path::Path::new("/nope"), None), None);
    }

    #[test]
    fn a_uv_package_gets_the_python_adapter() {
        let package = Package::from_identity("uv:acme", "packages/acme");
        let sync = for_package(&package).expect("uv has an adapter");
        assert_eq!(sync.locate(std::path::Path::new("/nope"), None), None);
    }

    #[test]
    fn a_single_package_repo_is_probed_for_every_adapter() {
        assert!(for_package(&Package::single("momo")).is_some());
    }

    #[test]
    fn an_ecosystem_without_an_adapter_is_skipped() {
        let package = Package::from_identity("node:@acme/ui", "packages/ui");
        assert!(for_package(&package).is_none());
    }

    #[test]
    fn a_bare_identity_has_no_provider() {
        assert_eq!(provider("momo"), None);
        assert_eq!(provider("cargo:momo"), Some("cargo"));
        assert_eq!(provider("uv:acme"), Some("uv"));
    }

    #[test]
    fn a_single_package_repo_with_only_a_pyproject_probes_python() {
        let dir = TempDir::new().expect("temp dir");
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nversion = \"1.0.0\"\n",
        )
        .expect("seed the manifest");
        let sync = Probe::new();

        let manifest = sync.locate(dir.path(), None).expect("a manifest is found");
        assert_eq!(manifest, dir.path().join("pyproject.toml"));
        let plan = sync
            .render(&manifest, &Version::new(2, 0, 0))
            .expect("render");
        assert_eq!(plan.writes.len(), 1, "{plan:?}");
    }

    #[test]
    fn a_single_package_repo_with_both_manifests_probes_cargo_first() {
        let dir = TempDir::new().expect("temp dir");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nversion = \"1.0.0\"\n",
        )
        .expect("seed Cargo.toml");
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nversion = \"1.0.0\"\n",
        )
        .expect("seed pyproject.toml");
        let sync = Probe::new();

        assert_eq!(
            sync.locate(dir.path(), None),
            Some(dir.path().join("Cargo.toml"))
        );
    }

    #[test]
    fn an_override_names_the_file_an_adapter_owns() {
        let dir = TempDir::new().expect("temp dir");
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nversion = \"1.0.0\"\n",
        )
        .expect("seed Cargo.toml");
        std::fs::write(
            dir.path().join("pyproject.toml"),
            "[project]\nversion = \"1.0.0\"\n",
        )
        .expect("seed pyproject.toml");
        let sync = Probe::new();

        let manifest = sync
            .locate(dir.path(), Some(Path::new("pyproject.toml")))
            .expect("the override is found");
        let plan = sync
            .render(&manifest, &Version::new(2, 0, 0))
            .expect("render");
        assert_eq!(plan.writes.len(), 1, "{plan:?}");
        assert!(
            plan.writes[0].text.contains("version = \"2.0.0\""),
            "{plan:?}"
        );
    }
}
