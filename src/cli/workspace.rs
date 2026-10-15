//! Building the Workspace: discovery through the task environment, with a
//! single-Package fallback.
//!
//! A repo with a mise project graph is a monorepo of Packages; a repo without
//! one — or one whose graph cannot be read — is treated as a single Package, so
//! momo's pre-monorepo behaviour is preserved exactly.

use std::env;
use std::path::Path;

use crate::domain::config::Config;
use crate::domain::package::Package;
use crate::domain::taskenv::TaskEnvError;
use crate::domain::workspace::{DefaultTarget, Workspace};
use anyhow::{Context, Result};

use crate::cli::Cli;
use crate::cli::error::CliError;

/// A repo's Packages, plus why a configured monorepo's graph could not be read.
///
/// Discovery is best-effort. When it fails, `workspace` is the single-Package
/// fallback and `unavailable` records the reason — but only when the mise
/// configuration declares a monorepo. In a repo that simply is not a monorepo
/// the graph's refusal is expected, so nothing is recorded and nothing is said.
pub struct Discovery {
    workspace: Workspace,
    unavailable: Option<TaskEnvError>,
}

impl Discovery {
    /// The resolved Workspace — the single-Package fallback when discovery
    /// failed.
    ///
    /// A verb reaches the discovered dependency graph through
    /// [`Workspace::graph`].
    pub fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    /// A repo whose Packages were resolved, or that is not a monorepo at all.
    fn resolved(workspace: Workspace) -> Self {
        Self {
            workspace,
            unavailable: None,
        }
    }

    /// The fallback for a graph momo could not read.
    ///
    /// A repo without `monorepo_root = true` is not a monorepo, so the failure
    /// is expected and stays silent; a configured monorepo's failure is kept.
    fn degraded(root: &Path, workspace: Workspace, error: TaskEnvError) -> Self {
        if crate::adapters::taskenv::is_monorepo(root) {
            Self {
                workspace,
                unavailable: Some(error),
            }
        } else {
            Self::resolved(workspace)
        }
    }
}

/// Load the Workspace for the repo at `root`.
///
/// Discovery goes through `mise tasks graph --json`. With no task environment
/// there is nothing to discover, so momo silently falls back to a single
/// Package. When a monorepo's graph cannot be read the fallback is recorded, so
/// the caller can warn — or fail, when packages were explicitly requested. A
/// graph that resolves to Packages with colliding slugs is fatal, not a
/// degraded discovery: the collision would silently share a tag prefix and a
/// Fragment store between two Packages.
pub fn load(root: &Path, config: &Config) -> Result<Discovery> {
    let single = || Workspace::single(root, &root_name(root), config);

    if crate::adapters::taskenv::detect(root).is_none() {
        return Ok(Discovery::resolved(single()));
    }

    match crate::adapters::taskenv::discover(root, config.command_timeout) {
        Ok(graph) if !graph.is_empty() => {
            let packages: Vec<Package> = graph
                .projects()
                .iter()
                .map(|project| Package::from_identity(project.id(), project.root()))
                .collect();
            let workspace = Workspace::new(root, packages, graph, config)?;
            Ok(Discovery::resolved(workspace))
        }
        Ok(_) => Ok(Discovery::degraded(root, single(), empty_graph())),
        Err(err) => Ok(Discovery::degraded(root, single(), err)),
    }
}

/// Resolve the Packages a command targets, reading the current directory.
///
/// An unreadable monorepo graph is reported here: loudly, because `-p`/`--all`
/// asked for Packages momo could not enumerate; as a warning otherwise, because
/// momo can still act on the single-Package fallback.
pub fn select<'a>(
    discovery: &'a Discovery,
    cli: &Cli,
    default: DefaultTarget,
) -> Result<Vec<&'a Package>> {
    if let Some(err) = &discovery.unavailable {
        if cli.package.is_some() || cli.all {
            return Err(CliError::DiscoveryFailed {
                detail: err.to_string(),
            }
            .into());
        }
        eprintln!(
            "warning: package discovery unavailable ({err}); treating this as a single-package repo"
        );
    }
    let cwd = env::current_dir().context("resolving the current directory")?;
    Ok(discovery
        .workspace()
        .select(cli.package.as_deref(), cli.all, default, &cwd)?)
}

/// The error for a monorepo whose graph listed no projects.
fn empty_graph() -> TaskEnvError {
    TaskEnvError::Graph {
        source: crate::domain::taskenv::GraphError::Empty,
    }
}

/// The name of the project root directory, or `package` when it has none.
fn root_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package")
        .to_owned()
}
