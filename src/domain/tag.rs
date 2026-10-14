//! The Go-resolvable tag prefix, and the warning when momo's is not it.
//!
//! A Go module's version *is* the git tag, and Go resolves a module version by
//! its **directory**, not its module name: a submodule at `api/` must be tagged
//! `api/v1.3.0`, a module at the repo root `v1.3.0`. momo namespaces a monorepo
//! Package's tag with its slug, so `go:example.com/acme/api` is tagged
//! `go-example-com-acme-api-v1.3.0` — a tag `go get` cannot resolve. This module
//! derives the prefix Go *would* resolve, purely from the identity and the
//! Package's directory, and words the warning. Changing the tag momo writes is a
//! separate, later change; this is the warning only.

use std::path::Path;

use crate::domain::package::Package;

/// The ecosystem prefix of an identity, e.g. `go` for `go:example.com/acme/api`.
fn provider(identity: &str) -> Option<&str> {
    identity.split_once(':').map(|(provider, _)| provider)
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
pub fn go_resolvable_prefix(package: &Package, base: &str) -> String {
    if at_repo_root(package) {
        base.to_owned()
    } else {
        format!("{}/{base}", package.relative_root().display())
    }
}

/// A warning when `package`'s tag prefix would be unresolvable by `go get`.
///
/// `None` for a package that is not a `go:` module, and for a module at the repo
/// root: Go resolves a root module as `<base><version>`, with no directory
/// prefix. The warning is for a `go:` module in a subdirectory, which momo
/// namespaces by its slug — `go-example-com-acme-api-v1.3.0`, which `go get`
/// cannot resolve — rather than by its directory (`api/v1.3.0`).
pub fn go_prefix_warning(package: &Package, base: &str) -> Option<String> {
    if provider(package.identity()) != Some("go") || at_repo_root(package) {
        return None;
    }
    let resolvable = go_resolvable_prefix(package, base);
    let computed = package.tag_prefix(base);
    if computed == resolvable {
        return None;
    }
    Some(format!(
        "`{}` is a Go module, but momo tags it `{computed}<version>`, which `go get` \
         cannot resolve; Go resolves a module version by its directory, so it must be \
         tagged `{resolvable}<version>`",
        package.identity()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_resolvable_prefix_is_the_directory_then_the_base() {
        assert_eq!(
            go_resolvable_prefix(
                &Package::from_identity("go:example.com/acme/api", "api"),
                "v"
            ),
            "api/v"
        );
        assert_eq!(
            go_resolvable_prefix(
                &Package::from_identity("go:example.com/acme/api", "api"),
                "rel-"
            ),
            "api/rel-"
        );
        assert_eq!(
            go_resolvable_prefix(&Package::from_identity("go:example.com/acme", "."), "v"),
            "v"
        );
        assert_eq!(go_resolvable_prefix(&Package::single("acme"), "v"), "v");
    }

    #[test]
    fn a_go_submodule_warns_that_its_tag_is_unresolvable() {
        let package = Package::from_identity("go:example.com/acme/api", "api");
        let warning = go_prefix_warning(&package, "v").expect("a warning");
        assert!(warning.contains("go get"), "{warning}");
        assert!(warning.contains("go-example-com-acme-api-v"), "{warning}");
        assert!(warning.contains("api/v"), "{warning}");
    }

    #[test]
    fn a_root_module_does_not_warn() {
        // A Go module at the repo root: Go tags it `v<version>`, with no
        // directory prefix, so there is no monorepo prefix to warn about.
        let package = Package::from_identity("go:example.com/acme", ".");
        assert_eq!(go_prefix_warning(&package, "v"), None);
    }

    #[test]
    fn a_non_go_package_is_silent() {
        let package = Package::from_identity("cargo:momo", "crates/momo");
        assert_eq!(go_prefix_warning(&package, "v"), None);
    }
}
