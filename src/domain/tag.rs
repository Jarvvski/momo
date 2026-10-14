//! The tag prefix a Package's versions are tagged under.
//!
//! A Package's tag is `<prefix><version>`, and this module derives that prefix
//! from the Package. Two rules apply. A `go:` module's version *is* the git tag,
//! and Go resolves a module version by its **directory**, not its module name:
//! a submodule at `api/` must be tagged `api/v1.3.0`, a module at the repo root
//! `v1.3.0`. Every other Package namespaces its tag with its slug, so
//! `cargo:momo` is tagged `cargo-momo-v1.3.0`. A configured per-package
//! `tag_prefix` overrides both, and is the escape hatch for a scheme momo does
//! not derive.
//!
//! The `/vN` major suffix lives in a Go module path, never in the tag, so the
//! prefix is major-agnostic. [`go_major_warning`] words the one thing Go
//! requires of the path itself.

use std::path::Path;

use crate::domain::package::Package;
use crate::domain::version::Version;

/// The ecosystem prefix of an identity, e.g. `go` for `go:example.com/acme/api`.
fn provider(identity: &str) -> Option<&str> {
    identity.split_once(':').map(|(provider, _)| provider)
}

/// The name after the provider namespace, e.g. `example.com/acme/api`.
fn module_name(identity: &str) -> &str {
    identity.split_once(':').map_or(identity, |(_, name)| name)
}

/// Whether `package` sits at the repository root.
fn at_repo_root(package: &Package) -> bool {
    let root = package.relative_root();
    root.as_os_str().is_empty() || root == Path::new(".")
}

/// The tag prefix Go resolves for `package`'s module, given momo's `base`.
///
/// Go resolves a module version by the module's directory relative to the repo
/// root: a submodule rooted at `R` is tagged `<R>/<base>`, and a module at the
/// repo root is tagged `<base>` alone. The `/vN` major suffix lives in the
/// module path, never in the tag, so the prefix is major-agnostic.
fn go_resolvable_prefix(package: &Package, base: &str) -> String {
    if at_repo_root(package) {
        base.to_owned()
    } else {
        format!("{}/{base}", package.relative_root().display())
    }
}

/// The prefix `package`'s versions are tagged under, given momo's `base`.
///
/// A configured per-package `tag_prefix` wins outright, because it is the
/// escape hatch for a naming scheme momo does not derive. Otherwise a `go:`
/// module is prefixed by its directory (see `go_resolvable_prefix`) and every
/// other Package by its slug — `<slug>-<base>` in a monorepo, `<base>` alone for
/// a single package.
pub fn prefix(package: &Package, base: &str) -> String {
    if let Some(configured) = package.tag_prefix_override() {
        return configured.to_owned();
    }
    if provider(package.identity()) == Some("go") {
        return go_resolvable_prefix(package, base);
    }
    package.slug_prefix(base)
}

/// The Go module path of a `go:` Package, e.g. `example.com/acme/api`.
///
/// `None` for any other ecosystem: only a Go module has a module path, and only
/// a Go module's major version is encoded in one.
pub fn go_module_path(package: &Package) -> Option<&str> {
    let identity = package.identity();
    (provider(identity) == Some("go")).then(|| module_name(identity))
}

/// A warning when tagging `version` cannot resolve as a proper Go major.
///
/// From major 2 onward, Go serves a version only when the **module path** ends
/// in `/vN` for that major: `v2.0.0` of `example.com/acme/api` resolves as
/// `v2.0.0+incompatible`, while `example.com/acme/api/v2` resolves properly.
/// Tagging `v2.0.0` is legitimate — only the path decides — so this is a
/// warning, not an error. `None` below major 2, and when the path already ends
/// in the matching suffix.
pub fn go_major_warning(module_path: &str, version: &Version) -> Option<String> {
    let major = version.major;
    if major < 2 {
        return None;
    }
    let suffix = format!("/v{major}");
    if module_path.ends_with(&suffix) {
        return None;
    }
    Some(format!(
        "`{module_path}` is being tagged `v{version}`, but its module path does not end in \
         `{suffix}`; Go resolves a v{major} version only from a `{suffix}` module path, so \
         `go get` serves this one as `+incompatible`"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::config::{Config, PackageConfig};

    #[test]
    fn a_go_submodule_is_prefixed_by_its_directory() {
        let package = Package::from_identity("go:example.com/acme/api", "api");
        assert_eq!(prefix(&package, "v"), "api/v");
        assert_eq!(prefix(&package, "rel-"), "api/rel-");
    }

    #[test]
    fn a_go_module_at_the_repo_root_takes_the_base_alone() {
        assert_eq!(
            prefix(&Package::from_identity("go:example.com/acme", "."), "v"),
            "v"
        );
        assert_eq!(prefix(&Package::single("acme"), "v"), "v");
    }

    #[test]
    fn a_non_go_package_is_still_namespaced_by_its_slug() {
        assert_eq!(
            prefix(&Package::from_identity("cargo:momo", "crates/momo"), "v"),
            "cargo-momo-v"
        );
    }

    #[test]
    fn a_configured_tag_prefix_overrides_the_derivation() {
        let mut config = Config::default();
        config.packages.insert(
            "go:example.com/acme/api".to_owned(),
            PackageConfig {
                tag_prefix: Some("api/v".to_owned()),
                ..PackageConfig::default()
            },
        );
        let package = Package::from_identity("go:example.com/acme/api", "api").configured(&config);
        assert_eq!(prefix(&package, "v"), "api/v");
    }

    #[test]
    fn only_a_go_package_has_a_module_path() {
        assert_eq!(
            go_module_path(&Package::from_identity("go:example.com/acme/api", "api")),
            Some("example.com/acme/api")
        );
        assert_eq!(
            go_module_path(&Package::from_identity("cargo:momo", "crates/momo")),
            None
        );
    }

    #[test]
    fn a_major_without_the_matching_module_suffix_warns() {
        let warning =
            go_major_warning("example.com/acme/api", &Version::new(2, 0, 0)).expect("a warning");
        assert!(warning.contains("/v2"), "{warning}");
        assert!(warning.contains("+incompatible"), "{warning}");
    }

    #[test]
    fn a_major_with_the_matching_module_suffix_is_silent() {
        assert_eq!(
            go_major_warning("example.com/acme/api/v2", &Version::new(2, 0, 0)),
            None
        );
    }

    #[test]
    fn a_version_below_major_two_never_warns() {
        assert_eq!(
            go_major_warning("example.com/acme/api", &Version::new(1, 4, 2)),
            None
        );
        assert_eq!(
            go_major_warning("example.com/acme/api", &Version::new(0, 3, 0)),
            None
        );
    }
}
