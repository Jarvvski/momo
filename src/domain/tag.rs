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

use std::path::{Path, PathBuf};

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

/// Whether `root`, a repo-relative directory, is the repository root itself.
fn at_repo_root(root: &Path) -> bool {
    root.as_os_str().is_empty() || root == Path::new(".")
}

/// The tag prefix Go resolves for `package`'s module, given momo's `base`.
///
/// Go resolves a module version by the module's directory relative to the repo
/// root: a submodule rooted at `R` is tagged `<R>/<base>`, and a module at the
/// repo root is tagged `<base>` alone. The `/vN` major suffix lives in the
/// module path, never in the tag, so the prefix is major-agnostic — see
/// [`go_module_subdirectory`].
fn go_resolvable_prefix(package: &Package, base: &str) -> String {
    let subdirectory = go_module_subdirectory(package);
    if at_repo_root(&subdirectory) {
        base.to_owned()
    } else {
        format!("{}/{base}", subdirectory.display())
    }
}

/// The module subdirectory a Go tag is prefixed by: `package`'s directory with a
/// trailing major-version component removed when the module path carries a
/// matching `/vN` suffix.
///
/// The `/vN` in a module path is a naming convention, not a directory Go
/// includes when resolving a tag. A module whose path is
/// `example.com/acme/api/v2` and whose go.mod sits in `api/v2/` is tagged
/// `api/v1.0.0`, not `api/v2/v1.0.0`; a root-level `v2` module is tagged
/// `v1.0.0` alone.
fn go_module_subdirectory(package: &Package) -> PathBuf {
    let root = package.relative_root();
    if at_repo_root(root) {
        return PathBuf::new();
    }
    let major = go_module_path(package).and_then(go_major_suffix);
    match (root.file_name().and_then(|name| name.to_str()), major) {
        (Some(dir), Some(suffix)) if dir == suffix => {
            root.parent().map_or_else(PathBuf::new, Path::to_path_buf)
        }
        _ => root.to_path_buf(),
    }
}

/// The trailing major-version component of a Go module path, e.g. `v2` for
/// `example.com/acme/api/v2`.
///
/// Go encodes major 2 and up in a module path as a trailing `/vN`. A `v0` or
/// `v1` component is never written as a suffix, so it yields `None`.
fn go_major_suffix(module_path: &str) -> Option<&str> {
    let last = module_path.rsplit('/').next()?;
    let major = last.strip_prefix('v')?;
    if major.is_empty()
        || major.starts_with('0')
        || !major.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    (major != "1").then_some(last)
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
/// From major 2 onward Go serves a version only when the **module path** encodes
/// that major: `example.com/acme/api/v2` resolves `v2.0.0`, while a `v2.0.0` tag
/// on `example.com/acme/api` does not resolve at all. Go falls back to
/// `+incompatible` only for a module with no go.mod, and a `go:` module
/// discovered by mise always has one, so the tag is simply unreachable.
/// `gopkg.in` encodes the major as a `.vN` suffix instead of `/vN`, so
/// `gopkg.in/yaml.v2` is already correct. Tagging `v2.0.0` is legitimate — only
/// the path decides — so this is a warning, not an error. `None` below major 2,
/// and when the path already encodes the matching major.
pub fn go_major_warning(module_path: &str, version: &Version) -> Option<String> {
    let major = version.major;
    if major < 2 {
        return None;
    }
    if module_path_encodes_major(module_path, major) {
        return None;
    }
    Some(format!(
        "`{module_path}` is being tagged `v{version}`, but its module path does not end in \
         `/v{major}`; Go resolves a v{major} version only from a `/v{major}` module path, so \
         this version will not resolve"
    ))
}

/// Whether `module_path` already encodes `major` as its major version.
///
/// A path-based module carries its major in a `/vN` suffix. `gopkg.in` encodes
/// it in a `.vN` suffix instead, which Go resolves the same way, so
/// `gopkg.in/yaml.v2` needs no rewrite.
fn module_path_encodes_major(module_path: &str, major: u64) -> bool {
    module_path.ends_with(&format!("/v{major}"))
        || (module_path.starts_with("gopkg.in/") && module_path.ends_with(&format!(".v{major}")))
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
    fn a_go_module_in_a_major_subdirectory_drops_the_vn_from_its_prefix() {
        let package = Package::from_identity("go:example.com/acme/api/v2", "api/v2");
        assert_eq!(prefix(&package, "v"), "api/v");
        assert_eq!(prefix(&package, "rel-"), "api/rel-");
    }

    #[test]
    fn a_root_level_major_module_takes_the_base_alone() {
        assert_eq!(
            prefix(&Package::from_identity("go:example.com/acme/v2", "v2"), "v"),
            "v"
        );
    }

    #[test]
    fn a_directory_that_does_not_match_the_module_major_is_kept() {
        let package = Package::from_identity("go:example.com/acme/api/v2", "api");
        assert_eq!(prefix(&package, "v"), "api/v");
    }

    #[test]
    fn only_a_major_of_two_or_more_is_a_module_path_suffix() {
        assert_eq!(go_major_suffix("example.com/acme/api"), None);
        assert_eq!(go_major_suffix("example.com/acme/v1"), None);
        assert_eq!(go_major_suffix("example.com/acme/v01"), None);
        assert_eq!(go_major_suffix("example.com/acme/v2"), Some("v2"));
        assert_eq!(go_major_suffix("example.com/acme/v10"), Some("v10"));
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
        assert!(!warning.contains("+incompatible"), "{warning}");
    }

    #[test]
    fn a_major_with_the_matching_module_suffix_is_silent() {
        assert_eq!(
            go_major_warning("example.com/acme/api/v2", &Version::new(2, 0, 0)),
            None
        );
    }

    #[test]
    fn a_gopkg_in_dot_suffix_satisfies_the_major() {
        assert_eq!(
            go_major_warning("gopkg.in/yaml.v2", &Version::new(2, 0, 0)),
            None
        );
    }

    #[test]
    fn a_gopkg_in_dot_suffix_of_another_major_still_warns() {
        assert!(
            go_major_warning("gopkg.in/yaml.v2", &Version::new(3, 0, 0)).is_some(),
            "a v3 tag on a .v2 path does not resolve"
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
