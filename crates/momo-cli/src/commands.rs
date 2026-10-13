//! Command handlers.
//!
//! Every verb on the command surface has a handler: the change verbs
//! (`break`/`add`/`fix`) record a Change, `status` reports the Unreleased
//! Changes and the Version they imply, `release`/`candidate` mint one, `tag`
//! creates the VCS tag, and `init`/`install` scaffold config and task wrappers.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use momo_core::taskenv::{InstallMode, InstallPlan, TaskEnvironment};
use momo_core::vcs::Vcs;
use semver::Version;
use time::Date;

use momo_core::change::{Change, Kind};
use momo_core::config::Config;
use momo_core::package::Package;
use momo_core::version::release_version;

use crate::cli::{Cli, Command};
use crate::config::{self, CONFIG_FILE};
use crate::recording;
use crate::release::{self, Outcome};
use crate::version;

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
        Command::Break { text } => record(cli, Kind::Break, text),
        Command::Add { text } => record(cli, Kind::Add, text),
        Command::Fix { text } => record(cli, Kind::Fix, text),
        Command::Status => status(),
        Command::Release => release(cli),
        Command::Candidate => candidate(cli),
        Command::Tag => tag(cli),
        Command::Init => init(cli),
        Command::Install {
            env,
            update,
            uninstall,
        } => install(cli, env.as_deref(), *update, *uninstall),
    }
}

/// Record one Change in the store the configuration selects.
///
/// A write command creates `momo.toml` on first use; with `--dry-run` nothing
/// is written at all.
fn record(cli: &Cli, kind: Kind, text: &str) -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;

    if !cli.dry_run {
        ensure_config(&root, &config)?;
    }

    let path = recording::record(&root, &config, Change::new(kind, text), cli.dry_run)?;
    if cli.dry_run {
        println!("would write {}", path.display());
    } else {
        println!("wrote {}", path.display());
    }
    Ok(())
}

/// Show the Unreleased Changes and the Version they imply.
fn status() -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;
    let package = Package::single(&root_name(&root));
    let vcs = detect_vcs(&root);

    let changes = recording::unreleased(&root, &config)?;
    let current = version::source(&root, &config, vcs.as_deref()).current()?;
    let next = release_version(&changes, &current).unwrap_or(current);

    let mut out = String::from("Unreleased\n\n");
    out.push_str(&format!("  {}\n", package.identity()));
    for kind in Kind::ALL {
        let matching: Vec<&Change> = changes
            .iter()
            .filter(|change| change.kind == kind)
            .collect();
        if matching.is_empty() {
            continue;
        }
        out.push_str(&format!("    {}\n", kind.heading()));
        for change in matching {
            out.push_str(&format!("      - {}\n", change.text));
        }
    }
    out.push_str(&format!("    → next: {next}\n"));
    print!("{out}");
    Ok(())
}

/// Mint a Release from the Unreleased Changes.
///
/// Writes the Changelog and `.momo/version` and consumes the Fragments; it never
/// commits and never tags.
fn release(cli: &Cli) -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;
    let vcs = detect_vcs(&root);

    let outcome = release::release(&root, &config, vcs.as_deref(), today(), cli.dry_run)?;
    report_release(&outcome, cli.dry_run, "release", "released");
    Ok(())
}

/// Cut a Release Candidate from the Unreleased Changes.
///
/// Bumps as a Release would, then appends `-rc.N`; the Fragments are shared with
/// the eventual Release.
fn candidate(cli: &Cli) -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;
    let vcs = detect_vcs(&root);

    let outcome = release::candidate(&root, &config, vcs.as_deref(), today(), cli.dry_run)?;
    report_release(&outcome, cli.dry_run, "cut", "cut");
    Ok(())
}

/// Create the VCS tag for the current Version.
fn tag(cli: &Cli) -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;
    let Some(vcs) = detect_vcs(&root) else {
        bail!("no VCS detected — nothing to tag");
    };

    let name = release::tag(&root, &config, vcs.as_ref(), cli.dry_run)?;
    let verb = if cli.dry_run { "would tag" } else { "tagged" };
    println!("{verb} {name}");
    Ok(())
}

/// Report what a `release` or `candidate` did, or would do under `--dry-run`.
fn report_release(outcome: &Outcome, dry_run: bool, verb: &str, done: &str) {
    let verb = if dry_run {
        format!("would {verb}")
    } else {
        done.to_owned()
    };
    println!("{verb} {}", outcome.version);

    let wrote = if dry_run { "would write" } else { "wrote" };
    println!("{wrote} {}", outcome.changelog.display());
    if let Some(path) = &outcome.version_file {
        println!("{wrote} {}", path.display());
    }

    let removed = if dry_run { "would remove" } else { "removed" };
    for path in &outcome.consumed {
        println!("{removed} {}", path.display());
    }
}

/// The repository backing `root`, when one exists.
fn detect_vcs(root: &Path) -> Option<Box<dyn Vcs>> {
    momo_vcs::detect(root).ok()
}

/// Today's date, in UTC.
fn today() -> Date {
    time::OffsetDateTime::now_utc().date()
}

/// Scaffold `momo.toml` at the project root.
///
/// Writes the effective configuration. With `--dry-run`, prints what would be
/// written and leaves the tree untouched.
fn init(cli: &Cli) -> Result<()> {
    let root = project_root()?;
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

/// The project root, discovered from the current directory.
fn project_root() -> Result<PathBuf> {
    let cwd = env::current_dir().context("resolving the current directory")?;
    Ok(config::project_root(&cwd))
}

/// Create `momo.toml` on first write, so the config is optional.
fn ensure_config(root: &Path, config: &Config) -> Result<()> {
    let path = root.join(CONFIG_FILE);
    if path.exists() {
        return Ok(());
    }
    fs::write(&path, config.to_string()).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

/// The name of the project root directory, or `package` when it has none.
fn root_name(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("package")
        .to_owned()
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
