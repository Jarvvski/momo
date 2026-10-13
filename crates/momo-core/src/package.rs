//! The Package: the unit of versioning.

use crate::slug::slugify;

/// A unit of versioning: one package in a repo.
///
/// A single-package repo is a monorepo with one Package. Discovery is deferred
/// to a later ticket; until then [`Package::single`] derives the only Package
/// from the repo's directory name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    identity: String,
    slug: String,
}

impl Package {
    /// Build a Package from its ecosystem identity and ref-safe slug.
    pub fn new(identity: impl Into<String>, slug: impl Into<String>) -> Self {
        Self {
            identity: identity.into(),
            slug: slug.into(),
        }
    }

    /// The only Package of a single-package repo named `name`.
    ///
    /// Until discovery lands the identity is the directory name and the slug is
    /// its path-safe form.
    pub fn single(name: &str) -> Self {
        Self::new(name, slugify(name))
    }

    /// The ecosystem identity, e.g. `cargo:momo`.
    pub fn identity(&self) -> &str {
        &self.identity
    }

    /// The ref/path-safe slug, e.g. `cargo-momo`.
    pub fn slug(&self) -> &str {
        &self.slug
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_package_derives_its_slug_from_its_name() {
        let package = Package::single("My Project");
        assert_eq!(package.identity(), "My Project");
        assert_eq!(package.slug(), "my-project");
    }
}
