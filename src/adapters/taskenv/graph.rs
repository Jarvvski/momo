//! Package discovery from the mise project graph.
//!
//! `mise tasks graph --json` lists each project's ecosystem ID and its directory
//! relative to the monorepo root; this module shells out to it and maps the
//! projects onto momo's [`Package`]s. The command requires `monorepo_root = true`
//! in the mise configuration and errors without it, so discovery is best-effort:
//! the caller degrades to a single Package when it fails.

use std::collections::HashMap;
use std::path::{Component, Path};

use crate::adapters::spawn::command;
use crate::domain::package::Package;
use crate::domain::taskenv::TaskEnvError;
use serde_json::Value;

/// The mise invocation that prints the project graph as JSON.
const ARGS: [&str; 3] = ["tasks", "graph", "--json"];

/// Discover the Packages of the monorepo rooted at `root`.
///
/// Errors when mise is absent, the repo does not set `monorepo_root = true`, or
/// the graph cannot be parsed.
pub fn discover(root: &Path) -> Result<Vec<Package>, TaskEnvError> {
    let output = command("mise", &ARGS, root)
        .output()
        .map_err(|source| TaskEnvError::Spawn {
            program: "mise".to_owned(),
            source,
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(TaskEnvError::Command {
            program: "mise".to_owned(),
            args: ARGS.join(" "),
            stderr: stderr.lines().next().unwrap_or_default().trim().to_owned(),
        });
    }

    let stdout = String::from_utf8(output.stdout).map_err(|err| {
        TaskEnvError::Graph(format!("`mise tasks graph` did not print UTF-8: {err}"))
    })?;
    parse_projects(&stdout)
}

/// Map a `mise tasks graph --json` document onto Packages.
///
/// The graph is a flat list in mise's array order, so it is deduplicated by
/// identity (the first occurrence wins) and each `root` is validated before it
/// reaches a Package: an absolute or `..`-escaping root would otherwise write
/// outside the repo.
fn parse_projects(json: &str) -> Result<Vec<Package>, TaskEnvError> {
    let document: Value = serde_json::from_str(json).map_err(|err| {
        TaskEnvError::Graph(format!("invalid JSON from `mise tasks graph`: {err}"))
    })?;
    let projects = document
        .get("projects")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            TaskEnvError::Graph("`mise tasks graph` printed no `projects` array".to_owned())
        })?;

    let mut packages = Vec::new();
    let mut roots: HashMap<String, String> = HashMap::new();
    for project in projects {
        let id = project
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| TaskEnvError::Graph("a project has no string `id`".to_owned()))?;
        let root = project
            .get("root")
            .and_then(Value::as_str)
            .ok_or_else(|| TaskEnvError::Graph(format!("project `{id}` has no string `root`")))?;
        validate_root(id, root)?;

        match roots.get(id) {
            Some(seen) if seen == root => continue,
            Some(seen) => {
                return Err(TaskEnvError::Graph(format!(
                    "project `{id}` is listed twice with different roots: `{seen}` and `{root}`"
                )));
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
        return Err(TaskEnvError::Graph(format!(
            "project `{id}` has an absolute `root` `{root}`"
        )));
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(TaskEnvError::Graph(format!(
            "project `{id}` has a `root` escaping the repo: `{root}`"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(err.to_string().contains("projects"), "{err}");
    }

    #[test]
    fn invalid_json_is_rejected() {
        let err = parse_projects("not json").expect_err("invalid JSON");
        assert!(err.to_string().contains("invalid JSON"), "{err}");
    }

    #[test]
    fn a_project_without_an_id_is_rejected() {
        let err =
            parse_projects(r#"{ "projects": [ { "root": "crates/a" } ] }"#).expect_err("no id");
        assert!(err.to_string().contains("`id`"), "{err}");
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
        assert!(err.to_string().contains("cargo:a"), "{err}");
        assert!(err.to_string().contains("different roots"), "{err}");
    }

    #[test]
    fn an_absolute_root_is_rejected() {
        let json = r#"{ "projects": [ { "id": "cargo:a", "root": "/tmp/elsewhere" } ] }"#;
        let err = parse_projects(json).expect_err("an absolute root");
        assert!(err.to_string().contains("absolute"), "{err}");
    }

    #[test]
    fn a_parent_escaping_root_is_rejected() {
        let json = r#"{ "projects": [ { "id": "cargo:a", "root": "../elsewhere" } ] }"#;
        let err = parse_projects(json).expect_err("a root escaping the repo");
        assert!(err.to_string().contains("escaping the repo"), "{err}");
    }
}
