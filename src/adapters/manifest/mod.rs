//! Manifest adapters: the Rust `Cargo.toml` in v0, implementing a port from
//! [`crate::domain::manifest`].

mod cargo;

#[cfg(test)]
mod roundtrip;

pub use cargo::CargoManifest;

use crate::domain::manifest::ManifestSync;
use crate::domain::package::Package;

/// The [`ManifestSync`] adapter for `package`, chosen by its ecosystem.
///
/// The ecosystem is the Package identity's prefix — `cargo:momo` is a Cargo
/// package. A single-package repo carries no prefix, so the sole v0 adapter is
/// probed: it finds nothing where there is no `Cargo.toml`. `None` when the
/// Package's ecosystem has no adapter yet, so a Release skips manifest sync.
pub fn for_package(package: &Package) -> Option<Box<dyn ManifestSync>> {
    match provider(package.identity()) {
        Some("cargo") | None => Some(Box::new(CargoManifest)),
        Some(_) => None,
    }
}

/// The ecosystem prefix of an identity, e.g. `cargo` for `cargo:momo`.
fn provider(identity: &str) -> Option<&str> {
    identity.split_once(':').map(|(provider, _)| provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cargo_package_gets_the_cargo_adapter() {
        let package = Package::from_identity("cargo:momo", "crates/momo");
        let sync = for_package(&package).expect("cargo has an adapter");
        assert_eq!(sync.locate(std::path::Path::new("/nope"), None), None);
    }

    #[test]
    fn a_single_package_repo_is_probed_for_cargo() {
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
    }
}
