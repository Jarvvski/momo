//! Manifest adapters: the Rust `Cargo.toml`, the Node `package.json` and the
//! Python `pyproject.toml`, each implementing a port from
//! [`crate::domain::manifest`].

mod cargo;
mod node;
mod python;
mod toml;

#[cfg(test)]
mod roundtrip;

pub use cargo::CargoManifest;
pub use node::NodeManifest;
pub use python::PythonManifest;

use std::path::{Path, PathBuf};

use crate::domain::manifest::{
    DependencyRequirement, ManifestEdits, ManifestError, ManifestPlan, ManifestSync, SkipReason,
};
use crate::domain::package::Package;
use semver::Version;

/// The [`ManifestSync`] adapter for `package`, chosen by its ecosystem.
///
/// The ecosystem is the Package identity's prefix — `cargo:momo` is a Cargo
/// package, `node:@acme/ui` a Node one, `uv:acme` a Python one. A single-package
/// repo carries no prefix, so every ecosystem's conventional manifest is probed
/// ([`Probe`]). `None` when the Package's ecosystem has no adapter yet, so a
/// Release skips manifest sync.
pub fn for_package(package: &Package) -> Option<Box<dyn ManifestSync>> {
    match provider(package.identity()) {
        Some("cargo") => Some(Box::new(CargoManifest)),
        Some("node") => Some(Box::new(NodeManifest)),
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
/// one present wins (Cargo before Node before Python, so a repo holding more
/// than one syncs its `Cargo.toml`). A configured `manifest` override names the
/// file directly; the adapter whose conventional manifest it is owns it, and
/// failing that the first adapter to accept it does.
struct Probe {
    /// The adapters probed, in order.
    adapters: Vec<Box<dyn ManifestSync>>,
}

impl Probe {
    /// The adapters probed, in order.
    fn new() -> Self {
        Self {
            adapters: vec![
                Box::new(CargoManifest),
                Box::new(NodeManifest),
                Box::new(PythonManifest),
            ],
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

    fn render(
        &self,
        manifest: &Path,
        edits: &ManifestEdits,
    ) -> Result<ManifestPlan, ManifestError> {
        match self.owner(manifest) {
            Some(adapter) => adapter.render(manifest, edits),
            None => Ok(unowned(manifest)),
        }
    }

    fn write(&self, manifest: &Path, edits: &ManifestEdits) -> Result<ManifestPlan, ManifestError> {
        match self.owner(manifest) {
            Some(adapter) => adapter.write(manifest, edits),
            None => Ok(unowned(manifest)),
        }
    }

    /// The requirements the adapter owning `manifest` reads; none when no
    /// adapter owns it.
    fn dependencies(&self, manifest: &Path) -> Result<Vec<DependencyRequirement>, ManifestError> {
        match self.owner(manifest) {
            Some(adapter) => adapter.dependencies(manifest),
            None => Ok(Vec::new()),
        }
    }

    /// A single-package repo carries no ecosystem prefix, so the range grammar
    /// is unknown until a manifest is located; momo reports it unchecked.
    fn satisfied(&self, _requirement: &str, _version: &Version) -> Option<bool> {
        None
    }

    /// A single-package repo carries no ecosystem prefix, so momo does not
    /// rewrite a range it cannot attribute to a grammar.
    fn widen(&self, _requirement: &str, _target: &Version) -> Option<String> {
        None
    }
}

/// A plan that writes nothing: no adapter owns `manifest`.
fn unowned(manifest: &Path) -> ManifestPlan {
    ManifestPlan {
        skipped: vec![(manifest.to_path_buf(), SkipReason::NoVersion)],
        ..ManifestPlan::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use tempfile::TempDir;

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    /// The edits that sync only a version, leaving every range alone.
    fn edits(text: &str) -> ManifestEdits {
        ManifestEdits::version(version(text))
    }

    #[test]
    fn a_cargo_package_gets_the_cargo_adapter() {
        let package = Package::from_identity("cargo:momo", "crates/momo");
        let sync = for_package(&package).expect("cargo has an adapter");
        assert_eq!(sync.locate(Path::new("/nope"), None), None);
    }

    #[test]
    fn a_node_package_gets_the_node_adapter() {
        let package = Package::from_identity("node:@acme/ui", "packages/ui");
        let sync = for_package(&package).expect("node has an adapter");
        assert_eq!(sync.locate(Path::new("/nope"), None), None);
    }

    #[test]
    fn a_uv_package_gets_the_python_adapter() {
        let package = Package::from_identity("uv:acme", "packages/acme");
        let sync = for_package(&package).expect("uv has an adapter");
        assert_eq!(sync.locate(Path::new("/nope"), None), None);
    }

    #[test]
    fn a_single_package_repo_is_probed_for_every_adapter() {
        assert!(for_package(&Package::single("momo")).is_some());
    }

    #[test]
    fn an_ecosystem_without_an_adapter_is_skipped() {
        let package = Package::from_identity("gem:rails", "gems/rails");
        assert!(for_package(&package).is_none());
    }

    #[test]
    fn a_single_package_repo_is_probed_for_cargo() {
        let dir = TempDir::new().expect("temp dir");
        fs::write(dir.path().join("Cargo.toml"), "").expect("seed the manifest");

        let sync = for_package(&Package::single("momo")).expect("a probe");
        assert_eq!(
            sync.locate(dir.path(), None),
            Some(dir.path().join("Cargo.toml"))
        );
    }

    #[test]
    fn a_single_package_repo_is_probed_for_node() {
        let dir = TempDir::new().expect("temp dir");
        fs::write(dir.path().join("package.json"), "{}").expect("seed the manifest");

        let sync = for_package(&Package::single("proj")).expect("a probe");
        assert_eq!(
            sync.locate(dir.path(), None),
            Some(dir.path().join("package.json"))
        );
    }

    #[test]
    fn a_single_package_repo_with_both_manifests_prefers_cargo() {
        let dir = TempDir::new().expect("temp dir");
        fs::write(dir.path().join("Cargo.toml"), "").expect("seed the Cargo manifest");
        fs::write(dir.path().join("package.json"), "{}").expect("seed the Node manifest");

        let sync = for_package(&Package::single("proj")).expect("a probe");
        assert_eq!(
            sync.locate(dir.path(), None),
            Some(dir.path().join("Cargo.toml"))
        );
    }

    #[test]
    fn the_probe_routes_a_located_manifest_to_its_ecosystem() {
        // A Node-only single-package repo: the probe locates `package.json`,
        // and the render must reach the Node adapter rather than Cargo.
        let dir = TempDir::new().expect("temp dir");
        let manifest = dir.path().join("package.json");
        fs::write(&manifest, "{\n  \"version\": \"0.1.0\"\n}\n").expect("seed the manifest");

        let sync = for_package(&Package::single("proj")).expect("a probe");
        let located = sync.locate(dir.path(), None).expect("a manifest");
        let plan = sync
            .render(&located, &edits("0.2.0"))
            .expect("render the manifest");

        assert_eq!(plan.writes.len(), 1, "{plan:?}");
        assert_eq!(plan.writes[0].text, "{\n  \"version\": \"0.2.0\"\n}\n");
    }

    #[test]
    fn a_conventional_override_still_routes_to_its_ecosystem() {
        // The override names the conventional manifest, so the probe routes the
        // render to Node even though Cargo is probed first.
        let dir = TempDir::new().expect("temp dir");
        let manifest = dir.path().join("package.json");
        fs::write(&manifest, "{\"version\": \"0.1.0\"}").expect("seed the manifest");

        let sync = for_package(&Package::single("proj")).expect("a probe");
        let located = sync
            .locate(dir.path(), Some(Path::new("package.json")))
            .expect("a manifest");
        let plan = sync
            .render(&located, &edits("0.2.0"))
            .expect("render the manifest");

        assert_eq!(plan.writes[0].text, "{\"version\": \"0.2.0\"}");
    }

    #[test]
    fn a_bare_identity_has_no_provider() {
        assert_eq!(provider("momo"), None);
        assert_eq!(provider("cargo:momo"), Some("cargo"));
        assert_eq!(provider("node:@acme/ui"), Some("node"));
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
            .render(&manifest, &ManifestEdits::version(Version::new(2, 0, 0)))
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
            .render(&manifest, &ManifestEdits::version(Version::new(2, 0, 0)))
            .expect("render");
        assert_eq!(plan.writes.len(), 1, "{plan:?}");
        assert!(
            plan.writes[0].text.contains("version = \"2.0.0\""),
            "{plan:?}"
        );
    }
}
