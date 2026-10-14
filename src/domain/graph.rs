//! The workspace dependency graph: the Projects a task runner discovered and the
//! dependency edges between them.
//!
//! The graph is inferred from the workspace manifests the task runner recognises
//! (Cargo, uv, Go, Node), not from configured roots. It is a pure data
//! structure: the adapter parses the runner's JSON into it, and the cascade
//! logic reads it to find the Projects that depend on a changed one. Nothing
//! here touches the filesystem.

use std::path::{Path, PathBuf};

/// A Project a task runner discovered in the workspace.
///
/// `manifest` is the file that declares the Project, `workspace_source` the
/// workspace file that listed it, and `provider` the ecosystem that recognised
/// it. All three are optional: an older task runner emits only `id` and `root`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    id: String,
    root: PathBuf,
    manifest: Option<PathBuf>,
    workspace_source: Option<PathBuf>,
    provider: Option<String>,
}

impl Project {
    /// Assemble a Project from its parts.
    pub fn new(
        id: impl Into<String>,
        root: impl Into<PathBuf>,
        manifest: Option<PathBuf>,
        workspace_source: Option<PathBuf>,
        provider: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            root: root.into(),
            manifest,
            workspace_source,
            provider,
        }
    }

    /// The ecosystem-prefixed identity, e.g. `cargo:momo`.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The Project's directory relative to the repo root (`.` for the root).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The manifest that declares the Project, when the graph named one.
    pub fn manifest(&self) -> Option<&Path> {
        self.manifest.as_deref()
    }

    /// The workspace file that listed the Project, when the graph named one.
    pub fn workspace_source(&self) -> Option<&Path> {
        self.workspace_source.as_deref()
    }

    /// The ecosystem that recognised the Project, when the graph named one.
    pub fn provider(&self) -> Option<&str> {
        self.provider.as_deref()
    }
}

/// A dependency edge: `dependent` depends on `dependency`.
///
/// `manifest` is the file that declared the edge, when the graph named one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edge {
    dependent: String,
    dependency: String,
    manifest: Option<PathBuf>,
}

impl Edge {
    /// Assemble an Edge from its parts.
    pub fn new(
        dependent: impl Into<String>,
        dependency: impl Into<String>,
        manifest: Option<PathBuf>,
    ) -> Self {
        Self {
            dependent: dependent.into(),
            dependency: dependency.into(),
            manifest,
        }
    }

    /// The Project that depends.
    pub fn dependent(&self) -> &str {
        &self.dependent
    }

    /// The Project depended on.
    pub fn dependency(&self) -> &str {
        &self.dependency
    }

    /// The manifest that declared the edge, when the graph named one.
    pub fn manifest(&self) -> Option<&Path> {
        self.manifest.as_deref()
    }
}

/// The Projects a workspace holds and the dependency edges between them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DependencyGraph {
    projects: Vec<Project>,
    edges: Vec<Edge>,
}

impl DependencyGraph {
    /// Assemble a graph from its Projects and Edges.
    pub fn new(projects: Vec<Project>, edges: Vec<Edge>) -> Self {
        Self { projects, edges }
    }

    /// The Projects, in discovery order.
    pub fn projects(&self) -> &[Project] {
        &self.projects
    }

    /// The dependency edges, in discovery order.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// Whether the graph holds no Projects.
    pub fn is_empty(&self) -> bool {
        self.projects.is_empty()
    }

    /// The edges of the Projects that depend directly on `id`.
    pub fn dependents_of(&self, id: &str) -> Vec<&Edge> {
        self.edges
            .iter()
            .filter(|edge| edge.dependency == id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A Project with no optional metadata, for the tests that do not need it.
    fn project(id: &str, root: &str) -> Project {
        Project::new(id, root, None, None, None)
    }

    #[test]
    fn dependents_of_returns_the_edges_pointing_at_a_project() {
        let graph = DependencyGraph::new(
            vec![
                project("cargo:a", "crates/a"),
                project("cargo:b", "crates/b"),
            ],
            vec![Edge::new(
                "cargo:b",
                "cargo:a",
                Some(PathBuf::from("crates/b/Cargo.toml")),
            )],
        );

        let dependents = graph.dependents_of("cargo:a");
        assert_eq!(dependents.len(), 1);
        assert_eq!(dependents[0].dependent(), "cargo:b");
        assert_eq!(
            dependents[0].manifest(),
            Some(Path::new("crates/b/Cargo.toml"))
        );
        assert!(graph.dependents_of("cargo:b").is_empty());
    }

    #[test]
    fn a_project_without_optional_metadata_reports_none() {
        let project = project("cargo:a", "crates/a");
        assert_eq!(project.manifest(), None);
        assert_eq!(project.workspace_source(), None);
        assert_eq!(project.provider(), None);
    }

    #[test]
    fn an_empty_graph_has_no_projects_or_edges() {
        let graph = DependencyGraph::default();
        assert!(graph.is_empty());
        assert!(graph.projects().is_empty());
        assert!(graph.edges().is_empty());
    }
}
