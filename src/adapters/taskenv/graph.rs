//! Package discovery from the mise project graph.
//!
//! `mise tasks graph --json` lists each project's ecosystem ID and its directory
//! relative to the monorepo root; this module shells out to it and maps the
//! projects onto momo's [`Package`]s. The command requires `monorepo_root = true`
//! in the mise configuration and errors without it, so discovery is best-effort:
//! the caller degrades to a single Package when it fails.

use std::collections::HashMap;
use std::path::{Component, Path};
use std::time::Duration;

use crate::adapters::spawn::{self, ProcessError};
use crate::domain::package::Package;
use crate::domain::taskenv::{GraphError, TaskEnvError};
use serde_json::Value;

/// The mise invocation that prints the project graph as JSON.
const ARGS: [&str; 3] = ["tasks", "graph", "--json"];

/// Discover the Packages of the monorepo rooted at `root`.
///
/// Errors when mise is absent, the repo does not set `monorepo_root = true`, or
/// the graph cannot be parsed. The mise command is bounded by `timeout`.
pub fn discover(root: &Path, timeout: Duration) -> Result<Vec<Package>, TaskEnvError> {
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
    parse_projects(&stdout)
}

/// Map a `mise tasks graph --json` document onto Packages.
///
/// The graph is a flat list in mise's array order, so it is deduplicated by
/// identity (the first occurrence wins) and each `root` is validated before it
/// reaches a Package: an absolute or `..`-escaping root would otherwise write
/// outside the repo.
fn parse_projects(json: &str) -> Result<Vec<Package>, TaskEnvError> {
    let document: Value = serde_json::from_str(json).map_err(|source| TaskEnvError::Graph {
        source: GraphError::Malformed { source },
    })?;
    let projects = document
        .get("projects")
        .and_then(Value::as_array)
        .ok_or(GraphError::MissingProjects)
        .map_err(|source| TaskEnvError::Graph { source })?;

    let mut packages = Vec::new();
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
                packages.push(Package::from_identity(id, root));
            }
        }
    }
    Ok(packages)
}

/// Reject a graph `root` that is absolute or escapes the repo.
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
    fn parses_the_project_ids_and_roots() {
        let json = r#"{
            "projects": [
                {
                    "id": "cargo:a",
                    "root": "crates/a",
                    "dependencies": [],
                    "metadata": { "workspace_source": "Cargo.toml" }
                },
                {
                    "id": "cargo:b",
                    "root": "crates/b",
                    "dependencies": ["cargo:a"]
                }
            ]
        }"#;

        let packages = parse_projects(json).expect("valid graph");
        assert_eq!(packages.len(), 2);
        assert_eq!(packages[0].identity(), "cargo:a");
        assert_eq!(packages[0].slug(), "cargo-a");
        assert_eq!(packages[0].relative_root(), Path::new("crates/a"));
        assert_eq!(packages[1].identity(), "cargo:b");
    }

    #[test]
    fn parses_the_root_project_at_dot() {
        let json = r#"{ "projects": [ { "id": "cargo:rootpkg", "root": "." } ] }"#;
        let packages = parse_projects(json).expect("valid graph");
        assert_eq!(packages[0].relative_root(), Path::new("."));
    }

    #[test]
    fn an_empty_project_list_is_allowed() {
        assert!(
            parse_projects(r#"{ "projects": [] }"#)
                .expect("valid graph")
                .is_empty()
        );
    }

    #[test]
    fn a_missing_projects_array_is_rejected() {
        let err = parse_projects(r#"{ "other": true }"#).expect_err("no projects array");
        assert!(matches!(graph_error(err), GraphError::MissingProjects));
    }

    #[test]
    fn invalid_json_is_rejected() {
        let err = parse_projects("not json").expect_err("invalid JSON");
        assert!(matches!(graph_error(err), GraphError::Malformed { .. }));
    }

    #[test]
    fn a_project_without_an_id_is_rejected() {
        let err =
            parse_projects(r#"{ "projects": [ { "root": "crates/a" } ] }"#).expect_err("no id");
        assert!(matches!(graph_error(err), GraphError::MissingId));
    }

    #[test]
    fn a_project_without_a_root_is_rejected() {
        let err =
            parse_projects(r#"{ "projects": [ { "id": "cargo:a" } ] }"#).expect_err("no root");
        assert!(matches!(
            graph_error(err),
            GraphError::MissingRoot { ref id } if id == "cargo:a"
        ));
    }

    #[test]
    fn a_duplicate_id_yields_one_package() {
        let json = r#"{
            "projects": [
                { "id": "cargo:a", "root": "crates/a" },
                { "id": "cargo:a", "root": "crates/a" }
            ]
        }"#;

        let packages = parse_projects(json).expect("valid graph");
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].identity(), "cargo:a");
    }

    #[test]
    fn a_duplicate_id_with_a_different_root_is_rejected() {
        let json = r#"{
            "projects": [
                { "id": "cargo:a", "root": "crates/a" },
                { "id": "cargo:a", "root": "crates/elsewhere" }
            ]
        }"#;

        let err = parse_projects(json).expect_err("a conflicting duplicate");
        assert!(matches!(
            graph_error(err),
            GraphError::Duplicate { ref id, ref first, ref second }
                if id == "cargo:a" && first == "crates/a" && second == "crates/elsewhere"
        ));
    }

    #[test]
    fn an_absolute_root_is_rejected() {
        let json = r#"{ "projects": [ { "id": "cargo:a", "root": "/tmp/elsewhere" } ] }"#;
        let err = parse_projects(json).expect_err("an absolute root");
        assert!(matches!(
            graph_error(err),
            GraphError::AbsoluteRoot { ref id, .. } if id == "cargo:a"
        ));
    }

    #[test]
    fn a_parent_escaping_root_is_rejected() {
        let json = r#"{ "projects": [ { "id": "cargo:a", "root": "../elsewhere" } ] }"#;
        let err = parse_projects(json).expect_err("a root escaping the repo");
        assert!(matches!(
            graph_error(err),
            GraphError::EscapingRoot { ref id, .. } if id == "cargo:a"
        ));
    }
}
