//! The cascade report: the dependent ranges a Release leaves unsatisfied.
//!
//! Report-only. For each Package being released, every dependency edge into it
//! (from the workspace's inferred graph) is checked against the Version about to
//! be minted. A Cargo dependent whose requirement the new Version does not
//! satisfy is reported; an edge momo cannot read — a Node/uv/Go dependent, or a
//! requirement whose syntax it does not recognise — is reported as *not checked*
//! rather than silently dropped, because a silent omission would read as
//! satisfied. Nothing here rewrites a manifest; the rewrite is a later change.

use std::path::Path;

use semver::Version;

use crate::adapters::manifest;
use crate::cli::output::Cascade;
use crate::domain::graph::DependencyGraph;
use crate::domain::manifest::{DependencyRequirement, ManifestSync};
use crate::domain::package::Package;
use crate::domain::workspace::Workspace;

/// The cascade entries releasing `package` at `target` would produce.
///
/// One entry per dependent edge that is unsatisfied at `target` or that momo
/// could not check. A satisfied edge produces nothing, so an empty result means
/// either no dependents or every one satisfied.
pub fn for_package(
    repo_root: &Path,
    workspace: &Workspace,
    package: &Package,
    target: &Version,
) -> Vec<Cascade> {
    let graph = workspace.graph();
    let Some(released) = project_id(graph, package) else {
        return Vec::new();
    };

    let mut entries = Vec::new();
    for edge in graph.dependents_of(released) {
        let dependent_id = edge.dependent();
        let Some(dependent) = package_of(workspace, dependent_id) else {
            continue;
        };

        // Only Cargo is checked for the first cut: a Node/uv/Go dependent
        // declares its ranges in a grammar momo does not read yet, so the edge
        // is reported unchecked rather than silently dropped.
        if !checks_requirements(dependent) {
            entries.push(unchecked(dependent_id, released, target));
            continue;
        }

        // No adapter at all for the dependent's ecosystem: its manifest cannot
        // be read, so the edge is reported unchecked rather than assumed fine.
        let Some(sync) = manifest::for_package(dependent) else {
            entries.push(unchecked(dependent_id, released, target));
            continue;
        };

        // The edge names the manifest that declared it; a graph that did not
        // name one falls back to the dependent's own conventional manifest.
        let Some(manifest) = edge
            .manifest()
            .map(|manifest| repo_root.join(manifest))
            .or_else(|| sync.locate(&dependent.dir(repo_root), dependent.manifest_override()))
        else {
            entries.push(unchecked(dependent_id, released, target));
            continue;
        };

        let Ok(requirements) = sync.dependencies(&manifest) else {
            entries.push(unchecked(dependent_id, released, target));
            continue;
        };

        if let Some(entry) =
            unsatisfied(dependent_id, released, target, sync.as_ref(), &requirements)
        {
            entries.push(entry);
        }
    }
    entries
}

/// The entry for `dependent`'s requirement on `released`, when it is not
/// satisfied at `target`.
///
/// `None` when every requirement the dependent declares for `released` is
/// satisfied — the satisfied case is reported by its absence. A requirement
/// momo cannot parse is checked with `satisfied == None`, so it is reported as
/// unchecked rather than dropped.
fn unsatisfied(
    dependent: &str,
    released: &str,
    target: &Version,
    sync: &dyn ManifestSync,
    requirements: &[DependencyRequirement],
) -> Option<Cascade> {
    let name = package_name(released);
    let matching: Vec<(&DependencyRequirement, Option<bool>)> = requirements
        .iter()
        .filter(|requirement| requirement.package == name)
        .map(|requirement| {
            (
                requirement,
                sync.satisfied(&requirement.requirement, target),
            )
        })
        .collect();

    // A dependency may be declared through more than one table; report the
    // unsatisfied reading in preference to an unchecked one.
    let (requirement, satisfied) = matching
        .iter()
        .find(|entry| entry.1 == Some(false))
        .or_else(|| matching.iter().find(|entry| entry.1.is_none()))?;

    Some(Cascade {
        dependent: dependent.to_owned(),
        dependency: released.to_owned(),
        requirement: Some(requirement.requirement.clone()),
        target: target.clone(),
        satisfied: *satisfied,
    })
}

/// An entry for an edge momo could not check.
fn unchecked(dependent: &str, released: &str, target: &Version) -> Cascade {
    Cascade {
        dependent: dependent.to_owned(),
        dependency: released.to_owned(),
        requirement: None,
        target: target.clone(),
        satisfied: None,
    }
}

/// The graph id of `package`, matched by identity and then by directory.
///
/// The graph's ids are mise's, so a configured `identity` override can hide the
/// match; a Package's directory cannot be overridden, so it is the fallback.
fn project_id<'a>(graph: &'a DependencyGraph, package: &Package) -> Option<&'a str> {
    graph
        .projects()
        .iter()
        .find(|project| project.id() == package.identity())
        .or_else(|| {
            graph
                .projects()
                .iter()
                .find(|project| project.root() == package.relative_root())
        })
        .map(|project| project.id())
}

/// The Package a graph id names, matched by identity and then by directory.
fn package_of<'a>(workspace: &'a Workspace, id: &str) -> Option<&'a Package> {
    workspace
        .packages()
        .iter()
        .find(|package| package.identity() == id)
        .or_else(|| {
            let root = workspace
                .graph()
                .projects()
                .iter()
                .find(|project| project.id() == id)?
                .root();
            workspace
                .packages()
                .iter()
                .find(|package| package.relative_root() == root)
        })
}

/// Whether momo evaluates `package`'s requirement ranges.
///
/// Cargo only, for the first cut: a `uv:` or `node:` dependent declares its
/// ranges in a grammar momo does not read yet, so its edges are reported
/// unchecked. A bare identity is a single-package repo's own Package, which no
/// dependency edge reaches.
fn checks_requirements(package: &Package) -> bool {
    matches!(
        package
            .identity()
            .split_once(':')
            .map(|(provider, _)| provider),
        Some("cargo") | None
    )
}

/// The bare package name a graph id names: `cargo:a` is `a`.
fn package_name(id: &str) -> &str {
    id.split_once(':').map_or(id, |(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::path::PathBuf;

    use crate::domain::config::Config;
    use crate::domain::graph::{Edge, Project};
    use tempfile::TempDir;

    /// A Project with only the fields the graph lookup reads.
    fn project(id: &str, root: &str) -> Project {
        Project::new(id, root, None, None, None)
    }

    /// A Workspace of `projects` and `edges`, with the config's defaults.
    fn workspace(root: &Path, projects: Vec<Project>, edges: Vec<Edge>) -> Workspace {
        let packages = projects
            .iter()
            .map(|project| Package::from_identity(project.id(), project.root()))
            .collect();
        Workspace::new(
            root,
            packages,
            DependencyGraph::new(projects, edges),
            &Config::default(),
        )
        .expect("the fixture's slugs are distinct")
    }

    /// A scratch repo with `crates/b/Cargo.toml` declaring `requirement` on `a`.
    fn cargo_repo(requirement: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("temp dir");
        let member = dir.path().join("crates/b");
        fs::create_dir_all(&member).expect("create the member dir");
        fs::write(
            member.join("Cargo.toml"),
            format!(
                "[package]\nname = \"b\"\nversion = \"0.1.0\"\n\n\
                 [dependencies]\na = {{ path = \"../a\", version = \"{requirement}\" }}\n"
            ),
        )
        .expect("seed the dependent manifest");
        (dir, member)
    }

    /// A two-crate Cargo workspace whose `b` depends on `a`.
    fn cargo_workspace(root: &Path) -> Workspace {
        workspace(
            root,
            vec![
                project("cargo:a", "crates/a"),
                project("cargo:b", "crates/b"),
            ],
            vec![Edge::new(
                "cargo:b",
                "cargo:a",
                Some(PathBuf::from("crates/b/Cargo.toml")),
            )],
        )
    }

    #[test]
    fn an_unsatisfied_cargo_range_is_reported() {
        let (dir, _) = cargo_repo("=0.0.1");
        let workspace = cargo_workspace(dir.path());
        let released = &workspace.packages()[0];

        let entries = for_package(dir.path(), &workspace, released, &Version::new(0, 1, 0));

        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].dependent, "cargo:b");
        assert_eq!(entries[0].dependency, "cargo:a");
        assert_eq!(entries[0].requirement.as_deref(), Some("=0.0.1"));
        assert_eq!(entries[0].target, Version::new(0, 1, 0));
        assert_eq!(entries[0].satisfied, Some(false));
    }

    #[test]
    fn a_satisfied_cargo_range_is_absent() {
        let (dir, _) = cargo_repo("^0.1.0");
        let workspace = cargo_workspace(dir.path());
        let released = &workspace.packages()[0];

        let entries = for_package(dir.path(), &workspace, released, &Version::new(0, 1, 0));

        assert!(entries.is_empty(), "{entries:?}");
    }

    #[test]
    fn a_non_cargo_dependent_is_reported_unchecked() {
        let dir = TempDir::new().expect("temp dir");
        let workspace = workspace(
            dir.path(),
            vec![
                project("node:@acme/a", "packages/a"),
                project("node:@acme/b", "packages/b"),
            ],
            vec![Edge::new("node:@acme/b", "node:@acme/a", None)],
        );
        let released = &workspace.packages()[0];

        let entries = for_package(dir.path(), &workspace, released, &Version::new(0, 1, 0));

        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].dependent, "node:@acme/b");
        assert_eq!(entries[0].requirement, None);
        assert_eq!(
            entries[0].satisfied, None,
            "an unchecked edge was reported as satisfied"
        );
    }

    #[test]
    fn a_python_dependent_is_reported_unchecked() {
        // A `uv:` dependent has a manifest adapter, but its PEP 440 ranges are
        // not evaluated yet — the edge must not disappear as though satisfied.
        let dir = TempDir::new().expect("temp dir");
        let workspace = workspace(
            dir.path(),
            vec![
                project("uv:acme", "packages/acme"),
                project("uv:ui", "packages/ui"),
            ],
            vec![Edge::new("uv:ui", "uv:acme", None)],
        );
        let released = &workspace.packages()[0];

        let entries = for_package(dir.path(), &workspace, released, &Version::new(0, 1, 0));

        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].dependent, "uv:ui");
        assert_eq!(entries[0].satisfied, None);
    }

    #[test]
    fn an_unreadable_dependent_manifest_is_reported_unchecked() {
        let dir = TempDir::new().expect("temp dir");
        let workspace = cargo_workspace(dir.path());
        let released = &workspace.packages()[0];

        // `crates/b/Cargo.toml` was never written, so the requirement cannot be
        // read — the edge must not vanish into a silent, apparently-satisfied
        // absence.
        let entries = for_package(dir.path(), &workspace, released, &Version::new(0, 1, 0));

        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].satisfied, None);
    }

    #[test]
    fn a_released_package_with_no_dependents_reports_nothing() {
        let dir = TempDir::new().expect("temp dir");
        let workspace = cargo_workspace(dir.path());
        let released = &workspace.packages()[1];

        assert!(for_package(dir.path(), &workspace, released, &Version::new(0, 1, 0)).is_empty());
    }
}
