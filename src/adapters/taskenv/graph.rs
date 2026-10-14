//! Package discovery from the mise project graph.
//!
//! `mise tasks graph --json` lists the workspace's projects and the dependency
//! edges between them. The graph is manifest-inferred: mise derives it from the
//! workspace manifests it recognises (Cargo, uv, Go, Node), so a project's
//! directory, the file that declares it, and the file that declares each edge
//! all come from mise — not from configured roots. This module shells out to it
//! and parses the result into a [`DependencyGraph`]. The command requires
//! `monorepo_root = true` in the mise configuration and errors without it, so
//! discovery is best-effort: the caller degrades to a single Package when it
//! fails.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use crate::adapters::spawn::{self, ProcessError};
use crate::domain::graph::{DependencyGraph, Edge, Project};
use crate::domain::taskenv::{GraphError, TaskEnvError};
use serde_json::Value;

/// The mise invocation that prints the project graph as JSON.
const ARGS: [&str; 3] = ["tasks", "graph", "--json"];

/// Discover the Projects and dependency edges of the monorepo rooted at `root`.
///
/// Errors when mise is absent, the repo does not set `monorepo_root = true`, or
/// the graph cannot be parsed. The mise command is bounded by `timeout`.
pub fn discover(root: &Path, timeout: Duration) -> Result<DependencyGraph, TaskEnvError> {
    let output = spawn::run("mise", &ARGS, root, timeout).map_err(TaskEnvError::from)?;

    if !output.success {
        return Err(TaskEnvError::from(ProcessError::Failed {
            program: "mise".to_owned(),
            args: ARGS.join(" "),
            stderr: output
                .stderr
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned(),
        }));
    }

    let stdout = output.stdout;
    parse_graph(&stdout)
}

/// Map a `mise tasks graph --json` document onto a [`DependencyGraph`].
///
/// Parsing is defensive: mise owns this format and it is moving, so a missing
/// optional field (`provenance`, `metadata`, `dependency_provenance`) degrades
/// to `None` rather than failing, and unknown fields are ignored. Only `id` and
/// `root` are required. The graph is a flat list in mise's array order, so it is
/// deduplicated by identity (the first occurrence wins) and every `root` and
/// `source` is validated before it is stored: an absolute or `..`-escaping path
/// would otherwise let a Project reach outside the repo.
fn parse_graph(json: &str) -> Result<DependencyGraph, TaskEnvError> {
    let document: Value = serde_json::from_str(json).map_err(|source| TaskEnvError::Graph {
        source: GraphError::Malformed { source },
    })?;
    let projects = document
        .get("projects")
        .and_then(Value::as_array)
        .ok_or(GraphError::MissingProjects)
        .map_err(|source| TaskEnvError::Graph { source })?;

    let mut graph_projects = Vec::new();
    let mut edges = Vec::new();
    let mut roots: HashMap<String, String> = HashMap::new();
    for project in projects {
        let id = project
            .get("id")
            .and_then(Value::as_str)
            .ok_or(GraphError::MissingId)
            .map_err(|source| TaskEnvError::Graph { source })?;
        let root = project
            .get("root")
            .and_then(Value::as_str)
            .ok_or_else(|| GraphError::MissingRoot { id: id.to_owned() })
            .map_err(|source| TaskEnvError::Graph { source })?;
        validate_root(id, root)?;

        match roots.get(id) {
            Some(seen) if seen == root => continue,
            Some(seen) => {
                return Err(TaskEnvError::Graph {
                    source: GraphError::Duplicate {
                        id: id.to_owned(),
                        first: seen.clone(),
                        second: root.to_owned(),
                    },
                });
            }
            None => {
                roots.insert(id.to_owned(), root.to_owned());
            }
        }

        let manifest = source(id, project, "provenance", "source")?;
        let workspace_source = source(id, project, "metadata", "workspace_source")?;
        let provider = project
            .get("provenance")
            .and_then(|provenance| provenance.get("provider"))
            .and_then(Value::as_str)
            .map(str::to_owned);

        edges.extend(dependencies(id, project)?);
        graph_projects.push(Project::new(
            id,
            root,
            manifest.map(PathBuf::from),
            workspace_source.map(PathBuf::from),
            provider,
        ));
    }
    Ok(DependencyGraph::new(graph_projects, edges))
}

/// Read a nested string `section.field` from a project, validating it as a
/// repo-relative path.
///
/// A missing field, or one that is not a string, degrades to `None`: mise owns
/// the format and may change it.
fn source<'a>(
    id: &str,
    project: &'a Value,
    section: &str,
    field: &str,
) -> Result<Option<&'a str>, TaskEnvError> {
    let value = project
        .get(section)
        .and_then(|section| section.get(field))
        .and_then(Value::as_str);
    if let Some(source) = value {
        validate_root(id, source)?;
    }
    Ok(value)
}

/// The dependency edges a project declares, deduplicated by dependency id.
///
/// `dependencies` is a bare edge list with no range, and `dependency_provenance`
/// names the file that declared each edge. A provider may report one dependency
/// through several kinds (a Cargo `[dependencies]` and `[dev-dependencies]`);
/// mise deduplicates them and so does this, so one dependency yields one edge.
fn dependencies(id: &str, project: &Value) -> Result<Vec<Edge>, TaskEnvError> {
    let Some(list) = project.get("dependencies").and_then(Value::as_array) else {
        return Ok(Vec::new());
    };
    let provenance = project.get("dependency_provenance");
    let mut seen = HashSet::new();
    let mut edges = Vec::new();
    for dependency in list {
        let Some(dependency) = dependency.as_str() else {
            continue;
        };
        if !seen.insert(dependency) {
            continue;
        }
        let manifest = provenance
            .and_then(|provenance| provenance.get(dependency))
            .and_then(|entry| entry.get("source"))
            .and_then(Value::as_str);
        if let Some(manifest) = manifest {
            validate_root(id, manifest)?;
        }
        edges.push(Edge::new(id, dependency, manifest.map(PathBuf::from)));
    }
    Ok(edges)
}

/// Reject a graph path (a `root` or a `source`) that is absolute or escapes the
/// repo.
fn validate_root(id: &str, root: &str) -> Result<(), TaskEnvError> {
    let path = Path::new(root);
    if path.is_absolute() {
        return Err(TaskEnvError::Graph {
            source: GraphError::AbsoluteRoot {
                id: id.to_owned(),
                root: root.to_owned(),
            },
        });
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(TaskEnvError::Graph {
            source: GraphError::EscapingRoot {
                id: id.to_owned(),
                root: root.to_owned(),
            },
        });
    }
    Ok(())
}

impl From<ProcessError> for TaskEnvError {
    fn from(err: ProcessError) -> Self {
        match err.timeout() {
            Some(timeout) => Self::Timeout { timeout },
            None => Self::Operation {
                source: Box::new(err),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The structured reason behind a graph failure.
    fn graph_error(err: TaskEnvError) -> GraphError {
        match err {
            TaskEnvError::Graph { source } => source,
            other => panic!("expected a graph error, got {other:?}"),
        }
    }

    #[test]
    fn parses_the_projects_edges_and_provenance() {
        let json = r#"{
            "projects": [
                {
                    "id": "cargo:a",
                    "root": "crates/a",
                    "dependencies": [],
                    "metadata": { "workspace_source": "Cargo.toml" },
                    "provenance": { "provider": "cargo", "source": "crates/a/Cargo.toml" },
                    "dependency_provenance": {}
                },
                {
                    "id": "cargo:b",
                    "root": "crates/b",
                    "dependencies": ["cargo:a"],
                    "metadata": { "workspace_source": "Cargo.toml" },
                    "provenance": { "provider": "cargo", "source": "crates/b/Cargo.toml" },
                    "dependency_provenance": {
                        "cargo:a": { "provider": "cargo", "source": "crates/b/Cargo.toml" }
                    }
                }
            ]
        }"#;

        let graph = parse_graph(json).expect("valid graph");
        assert_eq!(graph.projects().len(), 2);

        let a = &graph.projects()[0];
        assert_eq!(a.id(), "cargo:a");
        assert_eq!(a.root(), Path::new("crates/a"));
        assert_eq!(a.manifest(), Some(Path::new("crates/a/Cargo.toml")));
        assert_eq!(a.workspace_source(), Some(Path::new("Cargo.toml")));
        assert_eq!(a.provider(), Some("cargo"));

        assert_eq!(graph.edges().len(), 1);
        let edge = &graph.edges()[0];
        assert_eq!(edge.dependent(), "cargo:b");
        assert_eq!(edge.dependency(), "cargo:a");
        assert_eq!(edge.manifest(), Some(Path::new("crates/b/Cargo.toml")));
        assert_eq!(graph.dependents_of("cargo:a").len(), 1);
    }

    #[test]
    fn parses_the_root_project_at_dot() {
        let json = r#"{ "projects": [ { "id": "cargo:rootpkg", "root": "." } ] }"#;
        let graph = parse_graph(json).expect("valid graph");
        assert_eq!(graph.projects()[0].root(), Path::new("."));
    }

    #[test]
    fn missing_optional_fields_degrade() {
        // An older mise emits only `id`, `root` and `dependencies`.
        let json = r#"{
            "projects": [
                { "id": "cargo:a", "root": "crates/a", "dependencies": [] },
                { "id": "cargo:b", "root": "crates/b", "dependencies": ["cargo:a"] }
            ]
        }"#;

        let graph = parse_graph(json).expect("valid graph");
        let a = &graph.projects()[0];
        assert_eq!(a.manifest(), None);
        assert_eq!(a.workspace_source(), None);
        assert_eq!(a.provider(), None);
        assert_eq!(graph.edges()[0].manifest(), None);
    }

    #[test]
    fn a_dependency_listed_twice_yields_one_edge() {
        // A Cargo project that depends on another through more than one kind is
        // deduplicated to a single edge.
        let json = r#"{
            "projects": [
                { "id": "cargo:a", "root": "crates/a" },
                {
                    "id": "cargo:b",
                    "root": "crates/b",
                    "dependencies": ["cargo:a", "cargo:a"],
                    "dependency_provenance": {
                        "cargo:a": { "source": "crates/b/Cargo.toml" }
                    }
                }
            ]
        }"#;

        let graph = parse_graph(json).expect("valid graph");
        assert_eq!(graph.edges().len(), 1);
        assert_eq!(
            graph.edges()[0].manifest(),
            Some(Path::new("crates/b/Cargo.toml"))
        );
    }

    #[test]
    fn an_empty_project_list_is_allowed() {
        assert!(
            parse_graph(r#"{ "projects": [] }"#)
                .expect("valid graph")
                .is_empty()
        );
    }

    #[test]
    fn a_missing_projects_array_is_rejected() {
        let err = parse_graph(r#"{ "other": true }"#).expect_err("no projects array");
        assert!(matches!(graph_error(err), GraphError::MissingProjects));
    }

    #[test]
    fn invalid_json_is_rejected() {
        let err = parse_graph("not json").expect_err("invalid JSON");
        assert!(matches!(graph_error(err), GraphError::Malformed { .. }));
    }

    #[test]
    fn a_project_without_an_id_is_rejected() {
        let err = parse_graph(r#"{ "projects": [ { "root": "crates/a" } ] }"#).expect_err("no id");
        assert!(matches!(graph_error(err), GraphError::MissingId));
    }

    #[test]
    fn a_project_without_a_root_is_rejected() {
        let err = parse_graph(r#"{ "projects": [ { "id": "cargo:a" } ] }"#).expect_err("no root");
        assert!(matches!(
            graph_error(err),
            GraphError::MissingRoot { ref id } if id == "cargo:a"
        ));
    }

    #[test]
    fn a_duplicate_id_yields_one_project() {
        let json = r#"{
            "projects": [
                { "id": "cargo:a", "root": "crates/a" },
                { "id": "cargo:a", "root": "crates/a" }
            ]
        }"#;

        let graph = parse_graph(json).expect("valid graph");
        assert_eq!(graph.projects().len(), 1);
        assert_eq!(graph.projects()[0].id(), "cargo:a");
    }

    #[test]
    fn a_duplicate_id_with_a_different_root_is_rejected() {
        let json = r#"{
            "projects": [
                { "id": "cargo:a", "root": "crates/a" },
                { "id": "cargo:a", "root": "crates/elsewhere" }
            ]
        }"#;

        let err = parse_graph(json).expect_err("a conflicting duplicate");
        assert!(matches!(
            graph_error(err),
            GraphError::Duplicate { ref id, ref first, ref second }
                if id == "cargo:a" && first == "crates/a" && second == "crates/elsewhere"
        ));
    }

    #[test]
    fn an_absolute_root_is_rejected() {
        let json = r#"{ "projects": [ { "id": "cargo:a", "root": "/tmp/elsewhere" } ] }"#;
        let err = parse_graph(json).expect_err("an absolute root");
        assert!(matches!(
            graph_error(err),
            GraphError::AbsoluteRoot { ref id, .. } if id == "cargo:a"
        ));
    }

    #[test]
    fn a_parent_escaping_root_is_rejected() {
        let json = r#"{ "projects": [ { "id": "cargo:a", "root": "../elsewhere" } ] }"#;
        let err = parse_graph(json).expect_err("a root escaping the repo");
        assert!(matches!(
            graph_error(err),
            GraphError::EscapingRoot { ref id, .. } if id == "cargo:a"
        ));
    }

    #[test]
    fn an_absolute_source_is_rejected() {
        let json = r#"{
            "projects": [
                {
                    "id": "cargo:a",
                    "root": "crates/a",
                    "provenance": { "source": "/tmp/Cargo.toml" }
                }
            ]
        }"#;

        let err = parse_graph(json).expect_err("an absolute source");
        assert!(matches!(
            graph_error(err),
            GraphError::AbsoluteRoot { ref id, .. } if id == "cargo:a"
        ));
    }
}
