//! Building the Workspace: discovery through the task environment, with a
//! single-Package fallback.
//!
//! A repo with a mise project graph is a monorepo of Packages; a repo without
//! one — or one whose graph cannot be read — is treated as a single Package, so
//! momo's pre-monorepo behaviour is preserved exactly.

use std::env;
use std::path::Path;

use anyhow::{Context, Result};
use momo_core::config::Config;
use momo_core::package::Package;
use momo_core::workspace::{DefaultTarget, Workspace};

use crate::cli::Cli;

/// Load the Workspace for the repo at `root`.
///
/// Discovery goes through `mise tasks graph --json`. With no task environment
/// there is nothing to discover, so momo silently falls back to a single
/// Package; when a task environment exists but the graph cannot be read, momo
/// warns and falls back the same way.
pub fn load(root: &Path, config: &Config) -> Workspace {
    if momo_taskenv::detect(root).is_none() {
        return Workspace::single(root, &root_name(root), config);
    }

    match momo_taskenv::discover(root) {
        Ok(packages) if !packages.is_empty() => Workspace::new(root, packages, config),
        Ok(_) => {
            eprintln!(
                "warning: package discovery found no packages; treating this as a single-package repo"
            );
            Workspace::single(root, &root_name(root), config)
        }
        Err(err) => {
            eprintln!(
                "warning: package discovery unavailable ({err}); treating this as a single-package repo"
            );
            Workspace::single(root, &root_name(root), config)
        }
    }
}

/// Resolve the Packages a command targets, reading the current directory.
pub fn select<'a>(
    workspace: &'a Workspace,
    cli: &Cli,
    default: DefaultTarget,
) -> Result<Vec<&'a Package>> {
    let cwd = env::current_dir().context("resolving the current directory")?;
    Ok(workspace.select(cli.package.as_deref(), cli.all, default, &cwd)?)
}

/// The name of the project root directory, or `package` when it has none.
fn root_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package")
        .to_owned()
}
