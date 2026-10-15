//! The cascade: the dependent ranges a Release leaves unsatisfied, and the ones
//! it rewrites.
//!
//! Report first. For each Package being released, every dependency edge into it
//! (from the workspace's inferred graph) is checked against the Version about to
//! be minted. A Cargo, Node, Python (uv) or Go dependent whose requirement the
//! new Version does not satisfy is reported; an edge momo cannot read — a
//! provider with no adapter, a requirement whose syntax it does not recognise,
//! or a manifest it could not parse — is reported as *not checked* rather than
//! silently dropped, because a silent omission would read as satisfied.
//!
//! The rewrite rides on the same read. A dependent that is *itself* in the
//! release set has a manifest momo is going to write anyway, so the range that
//! the new Version breaks can be widened in the same transaction. A dependent
//! outside the set is never touched: manufacturing a bump for it would invent a
//! Change the project never recorded.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use semver::Version;

use crate::adapters::manifest;
use crate::cli::output::Cascade;
use crate::domain::graph::{DependencyGraph, Edge};
use crate::domain::manifest::{DependencyRangeEdit, DependencyRequirement};
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
        let reading = read(repo_root, dependent, released, edge, target);
        if let Some(entry) = cascade_entry(dependent_id, released, target, &reading) {
            entries.push(entry);
        }
    }
    entries
}

/// The dependency-range rewrites a Release run would apply to its dependents.
///
/// Scope is the dependents *already in the release set*: a range is only ever
/// rewritten in a manifest momo was already going to write. A dependent outside
/// the set keeps its range and stays reported by [`for_package`].
#[derive(Debug, Default)]
pub struct RangeRewrites {
    /// Per dependent Package identity, the edits its manifest receives.
    edits: BTreeMap<String, Vec<DependencyRangeEdit>>,
}

impl RangeRewrites {
    /// Take the edits staged for `dependent`, leaving none behind.
    pub fn take(&mut self, dependent: &str) -> Vec<DependencyRangeEdit> {
        self.edits.remove(dependent).unwrap_or_default()
    }
}

/// The range rewrites releasing every Package in `planned` would apply.
///
/// `planned` is the whole release set — each Package with the Version it mints —
/// because whether a range is rewritten depends on a *different* Package's
/// target. Each ecosystem's adapter rewrites only the forms it understands: a
/// requirement whose syntax momo does not recognise is left exactly as the
/// project wrote it and reported by [`for_package`], never guessed at.
pub fn range_rewrites(
    repo_root: &Path,
    workspace: &Workspace,
    planned: &[(&Package, &Version)],
) -> RangeRewrites {
    let graph = workspace.graph();
    let set: BTreeSet<&str> = planned
        .iter()
        .map(|(package, _)| package.identity())
        .collect();
    let mut rewrites = RangeRewrites::default();

    for &(released, target) in planned {
        let Some(released_id) = project_id(graph, released) else {
            continue;
        };
        for edge in graph.dependents_of(released_id) {
            let dependent_id = edge.dependent();
            // Only a dependent already in the release set: the edit rides along
            // with a manifest momo was going to write anyway.
            if !set.contains(dependent_id) {
                continue;
            }
            let Some(dependent) = package_of(workspace, dependent_id) else {
                continue;
            };
            let Reading::Requirements(requirements) =
                read(repo_root, dependent, released_id, edge, target)
            else {
                continue;
            };
            let Some(sync) = manifest::for_package(dependent) else {
                continue;
            };
            for (requirement, satisfied) in requirements {
                if satisfied != Some(false) {
                    continue;
                }
                // A form momo cannot rewrite is left as the project wrote it and
                // reported by `for_package`, never guessed at.
                if sync.widen(&requirement.requirement, target).is_none() {
                    continue;
                }
                let edit = DependencyRangeEdit {
                    name: requirement.package,
                    requirement: requirement.requirement,
                    target: target.clone(),
                };
                let edits = rewrites.edits.entry(dependent_id.to_owned()).or_default();
                // A dependency declared twice with one requirement is one edit.
                if edits.contains(&edit) {
                    continue;
                }
                edits.push(edit);
            }
        }
    }
    rewrites
}

/// What momo read of a dependent's requirement on a released Package.
enum Reading {
    /// The declarations momo read on the released Package, each with whether it
    /// is satisfied at the target.
    Requirements(Vec<(DependencyRequirement, Option<bool>)>),
    /// The edge could not be read: an ecosystem momo does not evaluate, no
    /// adapter for the dependent, no manifest, or a manifest it could not parse.
    Unreadable,
}

/// Read `dependent`'s requirements on the Package `released` names.
fn read(
    repo_root: &Path,
    dependent: &Package,
    released: &str,
    edge: &Edge,
    target: &Version,
) -> Reading {
    // A provider momo has no adapter for — a `gem:` dependent — declares its
    // ranges in a grammar momo does not read, so the edge is reported unchecked
    // rather than silently dropped.
    if !checks_requirements(dependent) {
        return Reading::Unreadable;
    }

    // No adapter at all for the dependent's ecosystem: its manifest cannot be
    // read, so the edge is reported unchecked rather than assumed fine.
    let Some(sync) = manifest::for_package(dependent) else {
        return Reading::Unreadable;
    };

    // The edge names the manifest that declared it; a graph that did not name
    // one falls back to the dependent's own conventional manifest.
    let Some(manifest) = edge
        .manifest()
        .map(|manifest| repo_root.join(manifest))
        .or_else(|| sync.locate(&dependent.dir(repo_root), dependent.manifest_override()))
    else {
        return Reading::Unreadable;
    };

    let Ok(requirements) = sync.dependencies(&manifest) else {
        return Reading::Unreadable;
    };

    let name = package_name(released);
    Reading::Requirements(
        requirements
            .into_iter()
            .filter(|requirement| requirement.package == name)
            .map(|requirement| {
                let satisfied = sync.satisfied(&requirement.requirement, target);
                (requirement, satisfied)
            })
            .collect(),
    )
}

/// The cascade entry `reading` produces, when it is not a satisfied edge.
///
/// `None` when every requirement the dependent declares for `released` is
/// satisfied — the satisfied case is reported by its absence. A requirement momo
/// cannot parse is carried with `satisfied == None`, so it is reported as
/// unchecked rather than dropped.
fn cascade_entry(
    dependent: &str,
    released: &str,
    target: &Version,
    reading: &Reading,
) -> Option<Cascade> {
    let Reading::Requirements(requirements) = reading else {
        return Some(unchecked(dependent, released, target));
    };

    // A dependency may be declared through more than one table; report the
    // unsatisfied reading in preference to an unchecked one.
    let (requirement, satisfied) = requirements
        .iter()
        .find(|(_, satisfied)| *satisfied == Some(false))
        .or_else(|| {
            requirements
                .iter()
                .find(|(_, satisfied)| satisfied.is_none())
        })?;

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
/// Cargo, Node, Python (`uv`) and Go: each has an adapter that reads its
/// manifest and evaluates its requirement grammar. A provider with no adapter —
/// a `gem:` dependent — is reported unchecked. A bare identity is a
/// single-package repo's own Package, which no dependency edge reaches.
fn checks_requirements(package: &Package) -> bool {
    matches!(
        package
            .identity()
            .split_once(':')
            .map(|(provider, _)| provider),
        Some("cargo" | "node" | "uv" | "go") | None
    )
}

/// The bare package name a graph id names: `cargo:a` is `a`.
pub(crate) fn package_name(id: &str) -> &str {
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

    /// A scratch repo with `packages/b/package.json` declaring `requirement` on
    /// `@acme/a`.
    fn node_repo(requirement: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("temp dir");
        let member = dir.path().join("packages/b");
        fs::create_dir_all(&member).expect("create the member dir");
        fs::write(
            member.join("package.json"),
            format!(
                "{{\n  \"name\": \"@acme/b\",\n  \"version\": \"0.1.0\",\n  \
                 \"dependencies\": {{ \"@acme/a\": \"{requirement}\" }}\n}}\n"
            ),
        )
        .expect("seed the dependent manifest");
        (dir, member)
    }

    /// A two-package Node workspace whose `b` depends on `a`.
    fn node_workspace(root: &Path) -> Workspace {
        workspace(
            root,
            vec![
                project("node:@acme/a", "packages/a"),
                project("node:@acme/b", "packages/b"),
            ],
            vec![Edge::new("node:@acme/b", "node:@acme/a", None)],
        )
    }

    /// A scratch repo with `packages/ui/pyproject.toml` declaring `specifier`
    /// on `acme`.
    fn python_repo(specifier: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("temp dir");
        let member = dir.path().join("packages/ui");
        fs::create_dir_all(&member).expect("create the member dir");
        fs::write(
            member.join("pyproject.toml"),
            format!(
                "[project]\nname = \"ui\"\nversion = \"0.1.0\"\n\
                 dependencies = [\"acme{specifier}\"]\n"
            ),
        )
        .expect("seed the dependent manifest");
        (dir, member)
    }

    /// A two-package Python workspace whose `ui` depends on `acme`.
    fn python_workspace(root: &Path) -> Workspace {
        workspace(
            root,
            vec![
                project("uv:acme", "packages/acme"),
                project("uv:ui", "packages/ui"),
            ],
            vec![Edge::new("uv:ui", "uv:acme", None)],
        )
    }

    /// A scratch repo with `api2/go.mod` requiring `example.com/acme/api` at
    /// `requirement`.
    fn go_repo(requirement: &str) -> (TempDir, PathBuf) {
        let dir = TempDir::new().expect("temp dir");
        let member = dir.path().join("api2");
        fs::create_dir_all(&member).expect("create the member dir");
        fs::write(
            member.join("go.mod"),
            format!(
                "module example.com/acme/api2\n\ngo 1.22\n\n\
                 require example.com/acme/api {requirement}\n"
            ),
        )
        .expect("seed the dependent manifest");
        (dir, member)
    }

    /// A two-module Go workspace whose `api2` depends on `api`.
    fn go_workspace(root: &Path) -> Workspace {
        workspace(
            root,
            vec![
                project("go:example.com/acme/api", "api"),
                project("go:example.com/acme/api2", "api2"),
            ],
            vec![Edge::new(
                "go:example.com/acme/api2",
                "go:example.com/acme/api",
                None,
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
    fn an_unsatisfied_node_range_is_reported_with_its_requirement() {
        let (dir, _) = node_repo("^0.1.0");
        let workspace = node_workspace(dir.path());
        let released = &workspace.packages()[0];

        // `^0.1.0` admits `0.1.x`, not `0.2.0`.
        let entries = for_package(dir.path(), &workspace, released, &Version::new(0, 2, 0));

        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].dependent, "node:@acme/b");
        assert_eq!(entries[0].requirement.as_deref(), Some("^0.1.0"));
        assert_eq!(entries[0].satisfied, Some(false));
    }

    #[test]
    fn a_satisfied_node_range_is_absent() {
        let (dir, _) = node_repo("^0.2.0");
        let workspace = node_workspace(dir.path());
        let released = &workspace.packages()[0];

        let entries = for_package(dir.path(), &workspace, released, &Version::new(0, 2, 0));

        assert!(entries.is_empty(), "{entries:?}");
    }

    #[test]
    fn an_unsatisfied_python_range_is_reported_with_its_requirement() {
        let (dir, _) = python_repo(">=0.1.0,<0.2.0");
        let workspace = python_workspace(dir.path());
        let released = &workspace.packages()[0];

        let entries = for_package(dir.path(), &workspace, released, &Version::new(0, 2, 0));

        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].dependent, "uv:ui");
        assert_eq!(entries[0].requirement.as_deref(), Some(">=0.1.0,<0.2.0"));
        assert_eq!(entries[0].satisfied, Some(false));
    }

    #[test]
    fn a_satisfied_python_range_is_absent() {
        let (dir, _) = python_repo(">=0.1.0");
        let workspace = python_workspace(dir.path());
        let released = &workspace.packages()[0];

        let entries = for_package(dir.path(), &workspace, released, &Version::new(0, 2, 0));

        assert!(entries.is_empty(), "{entries:?}");
    }

    #[test]
    fn a_go_dependent_is_reported_with_its_pinned_version() {
        // The pin `v0.1.0` is a different major from the target `1.0.0`, which
        // is a different module path, so the edge is unsatisfied.
        let (dir, _) = go_repo("v0.1.0");
        let workspace = go_workspace(dir.path());
        let released = &workspace.packages()[0];

        let entries = for_package(dir.path(), &workspace, released, &Version::new(1, 0, 0));

        assert_eq!(entries.len(), 1, "{entries:?}");
        assert_eq!(entries[0].dependent, "go:example.com/acme/api2");
        assert_eq!(entries[0].requirement.as_deref(), Some("v0.1.0"));
        assert_eq!(entries[0].satisfied, Some(false));
    }

    #[test]
    fn a_satisfied_go_pin_is_absent() {
        // `v0.2.0` keeps the pinned major and is at least `v0.1.0`.
        let (dir, _) = go_repo("v0.1.0");
        let workspace = go_workspace(dir.path());
        let released = &workspace.packages()[0];

        let entries = for_package(dir.path(), &workspace, released, &Version::new(0, 2, 0));

        assert!(entries.is_empty(), "{entries:?}");
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

    /// The edit that widens `a`'s `^0.1.0` to admit `0.2.0`.
    fn widening() -> DependencyRangeEdit {
        DependencyRangeEdit {
            name: "a".to_owned(),
            requirement: "^0.1.0".to_owned(),
            target: Version::new(0, 2, 0),
        }
    }

    #[test]
    fn a_dependent_in_the_release_set_gets_its_range_widened() {
        let (dir, _) = cargo_repo("^0.1.0");
        let workspace = cargo_workspace(dir.path());
        let a = &workspace.packages()[0];
        let b = &workspace.packages()[1];
        let (target_a, target_b) = (Version::new(0, 2, 0), Version::new(0, 1, 1));

        let mut rewrites =
            range_rewrites(dir.path(), &workspace, &[(a, &target_a), (b, &target_b)]);

        assert_eq!(rewrites.take("cargo:b"), vec![widening()]);
    }

    #[test]
    fn a_dependent_outside_the_release_set_is_never_rewritten() {
        // Scope (b): the edit rides along with a manifest momo was already
        // going to write. A dependent with no Changes of its own keeps its
        // range, and stays reported by `for_package` instead.
        let (dir, _) = cargo_repo("^0.1.0");
        let workspace = cargo_workspace(dir.path());
        let a = &workspace.packages()[0];
        let target_a = Version::new(0, 2, 0);

        let mut rewrites = range_rewrites(dir.path(), &workspace, &[(a, &target_a)]);

        assert!(rewrites.take("cargo:b").is_empty());
    }

    #[test]
    fn a_satisfied_range_is_not_rewritten() {
        // `^0.2.0` already admits the target, so there is nothing to widen.
        let (dir, _) = cargo_repo("^0.2.0");
        let workspace = cargo_workspace(dir.path());
        let a = &workspace.packages()[0];
        let b = &workspace.packages()[1];
        let (target_a, target_b) = (Version::new(0, 2, 0), Version::new(0, 1, 1));

        let mut rewrites =
            range_rewrites(dir.path(), &workspace, &[(a, &target_a), (b, &target_b)]);

        assert!(rewrites.take("cargo:b").is_empty());
    }

    #[test]
    fn an_unwritable_range_is_left_as_the_project_wrote_it() {
        // A conjunction is unsatisfied but not a form momo rewrites; guessing at
        // it would be a semver lie, so the range stays and the cascade reports it.
        let (dir, _) = cargo_repo(">=0.1.0, <0.2.0");
        let workspace = cargo_workspace(dir.path());
        let a = &workspace.packages()[0];
        let b = &workspace.packages()[1];
        let (target_a, target_b) = (Version::new(0, 2, 0), Version::new(0, 1, 1));

        let mut rewrites =
            range_rewrites(dir.path(), &workspace, &[(a, &target_a), (b, &target_b)]);

        assert!(rewrites.take("cargo:b").is_empty());
    }

    #[test]
    fn a_node_dependent_gets_its_range_widened() {
        let (dir, _) = node_repo("^0.1.0");
        let workspace = node_workspace(dir.path());
        let a = &workspace.packages()[0];
        let b = &workspace.packages()[1];
        let (target_a, target_b) = (Version::new(0, 2, 0), Version::new(0, 1, 1));

        let mut rewrites =
            range_rewrites(dir.path(), &workspace, &[(a, &target_a), (b, &target_b)]);

        assert_eq!(
            rewrites.take("node:@acme/b"),
            vec![DependencyRangeEdit {
                name: "@acme/a".to_owned(),
                requirement: "^0.1.0".to_owned(),
                target: Version::new(0, 2, 0),
            }]
        );
    }

    #[test]
    fn an_unsupported_ecosystem_is_never_rewritten() {
        // A `gem:` dependent has no adapter, so it is reported unchecked by
        // `for_package` and staged by no rewrite.
        let dir = TempDir::new().expect("temp dir");
        let workspace = workspace(
            dir.path(),
            vec![project("gem:a", "gems/a"), project("gem:b", "gems/b")],
            vec![Edge::new("gem:b", "gem:a", None)],
        );
        let a = &workspace.packages()[0];
        let b = &workspace.packages()[1];
        let (target_a, target_b) = (Version::new(0, 2, 0), Version::new(0, 1, 1));

        let mut rewrites =
            range_rewrites(dir.path(), &workspace, &[(a, &target_a), (b, &target_b)]);

        assert!(rewrites.take("gem:b").is_empty());
    }

    #[test]
    fn a_requirement_declared_twice_is_one_edit() {
        let dir = TempDir::new().expect("temp dir");
        let member = dir.path().join("crates/b");
        fs::create_dir_all(&member).expect("create the member dir");
        fs::write(
            member.join("Cargo.toml"),
            "[package]\nname = \"b\"\nversion = \"0.1.0\"\n\n\
             [dependencies]\na = { path = \"../a\", version = \"^0.1.0\" }\n\n\
             [dev-dependencies]\na = { path = \"../a\", version = \"^0.1.0\" }\n",
        )
        .expect("seed the dependent manifest");
        let workspace = cargo_workspace(dir.path());
        let a = &workspace.packages()[0];
        let b = &workspace.packages()[1];
        let (target_a, target_b) = (Version::new(0, 2, 0), Version::new(0, 1, 1));

        let mut rewrites =
            range_rewrites(dir.path(), &workspace, &[(a, &target_a), (b, &target_b)]);

        assert_eq!(rewrites.take("cargo:b"), vec![widening()]);
    }
}
