//! Command handlers.
//!
//! `init` and `install` have real handlers in v0; every other verb is declared
//! on the command surface and exits non-zero with a "not implemented" message.

use std::env;
use std::fs;

use anyhow::{Context, Result, bail};
use momo_core::taskenv::{InstallMode, InstallPlan, TaskEnvironment};
use semver::Version;

use crate::cli::{Cli, Command};
use crate::config::{self, CONFIG_FILE};

/// The momo verbs exposed as `momo:*` tasks, in command-surface order.
const VERBS: [&str; 9] = [
    "break",
    "add",
    "fix",
    "status",
    "release",
    "candidate",
    "tag",
    "init",
    "install",
];

/// The momo CLI version the generated wrappers pin.
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Dispatch a parsed command line.
pub fn run(cli: &Cli) -> Result<()> {
    match &cli.command {
        Command::Init => init(cli),
        Command::Install {
            env,
            update,
            uninstall,
        } => install(cli, env.as_deref(), *update, *uninstall),
        other => bail!("`momo {other}` is not implemented yet"),
    }
}

/// Scaffold `momo.toml` at the project root.
///
/// Writes the effective configuration. With `--dry-run`, prints what would be
/// written and leaves the tree untouched.
fn init(cli: &Cli) -> Result<()> {
    let cwd = env::current_dir().context("resolving the current directory")?;
    let root = config::project_root(&cwd);
    let path = root.join(CONFIG_FILE);

    if cli.dry_run {
        let config = config::load(&root)?;
        eprintln!("would write {}", path.display());
        print!("{config}");
        return Ok(());
    }

    if path.exists() {
        bail!("{} already exists; refusing to overwrite", path.display());
    }

    let config = config::load(&root)?;
    fs::write(&path, config.to_string()).with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {}", path.display());
    Ok(())
}

/// Install momo's task wrappers into a task environment.
///
/// Auto-detects mise at or above the project root, or uses the explicit `env`
/// override. With no environment and no override this is a warning and a no-op.
/// `update` refreshes the pinned version; `uninstall` removes what install wrote.
fn install(cli: &Cli, env: Option<&str>, update: bool, uninstall: bool) -> Result<()> {
    let cwd = env::current_dir().context("resolving the current directory")?;
    let root = config::project_root(&cwd);

    let environment = match env {
        Some(name) => momo_taskenv::named(name, &root)?,
        None => match momo_taskenv::detect(&root) {
            Some(environment) => environment,
            None => {
                eprintln!(
                    "warning: no task environment detected at or above {}; nothing to do",
                    root.display()
                );
                return Ok(());
            }
        },
    };

    if uninstall {
        return uninstall_momo(cli, environment.as_ref());
    }

    let version = Version::parse(VERSION).context("parsing the momo version")?;
    let mode = if update {
        InstallMode::Update
    } else {
        InstallMode::Install
    };
    let plan = environment.plan(&version, &VERBS, mode)?;

    if cli.dry_run {
        report_plan(&plan, environment.as_ref(), &version, true);
        return Ok(());
    }

    environment.write(&plan.writes)?;
    config::record_task_env(environment.root(), Some(environment.name()))?;
    report_plan(&plan, environment.as_ref(), &version, false);
    Ok(())
}

/// Remove momo's task wrappers and clear the recorded environment.
fn uninstall_momo(cli: &Cli, environment: &dyn TaskEnvironment) -> Result<()> {
    let dir = environment.tasks_dir();
    if cli.dry_run {
        println!("would remove {}", dir.display());
        return Ok(());
    }
    environment.uninstall()?;
    config::record_task_env(environment.root(), None)?;
    println!("removed {}", dir.display());
    Ok(())
}

/// Print what an install did, or would do under `--dry-run`.
fn report_plan(
    plan: &InstallPlan,
    environment: &dyn TaskEnvironment,
    version: &Version,
    dry_run: bool,
) {
    if !plan.writes.is_empty() {
        let outcome = if dry_run {
            "would install"
        } else {
            "installed"
        };
        println!(
            "{outcome} momo {version}: {} task(s) in {}",
            plan.writes.len(),
            environment.tasks_dir().display()
        );
    } else if plan.stale.is_empty() {
        println!(
            "momo {version} is already installed in {}",
            environment.tasks_dir().display()
        );
    }
    if !plan.stale.is_empty() {
        eprintln!(
            "warning: {} task(s) pin a different version; run `momo install --update`",
            plan.stale.len()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_command_has_an_installed_verb() {
        let commands = [
            Command::Break {
                text: String::new(),
            },
            Command::Add {
                text: String::new(),
            },
            Command::Fix {
                text: String::new(),
            },
            Command::Status,
            Command::Release,
            Command::Candidate,
            Command::Tag,
            Command::Init,
            Command::Install {
                env: None,
                update: false,
                uninstall: false,
            },
        ];
        for command in commands {
            let verb = command.to_string();
            assert!(
                VERBS.contains(&verb.as_str()),
                "no task wrapper for `{verb}`"
            );
        }
        assert_eq!(VERBS.len(), 9, "the verb list drifted from the surface");
    }
}
