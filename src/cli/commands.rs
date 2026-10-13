//! Command handlers.
//!
//! Every verb on the command surface has a handler: the change verbs
//! (`break`/`add`/`fix`) record a Change, `status` reports the Unreleased
//! Changes and the Version they imply, `release`/`candidate` mint one, `tag`
//! creates the VCS tag, and `init`/`install` scaffold config and task wrappers.
//!
//! Package-aware verbs resolve the Packages they act on through
//! [`crate::workspace`]: `-p` scopes to one, `--all` covers every one, and the
//! cwd-relative default applies otherwise. Each Package is reported through
//! [`crate::output`].

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::domain::taskenv::{InstallMode, InstallPlan, TaskEnvironment};
use crate::domain::vcs::Vcs;
use crate::domain::workspace::DefaultTarget;
use anyhow::{Context, Result, bail};
use semver::Version;
use time::Date;

use crate::domain::change::{Change, Kind};
use crate::domain::config::Config;
use crate::domain::version::{VersionSource, release_version};

use crate::cli::config::{self, CONFIG_FILE};
use crate::cli::output::{self, StatusPackage, Verb};
use crate::cli::recording;
use crate::cli::release::{self, Outcome};
use crate::cli::version::{self, FileVersionSource, VERSION_FILE};
use crate::cli::workspace;
use crate::cli::{Cli, Command};

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
    reject_output_flags(cli)?;
    match &cli.command {
        Command::Break { text } => record(cli, Kind::Break, text),
        Command::Add { text } => record(cli, Kind::Add, text),
        Command::Fix { text } => record(cli, Kind::Fix, text),
        Command::Status { check } => status(cli, *check),
        Command::Release => release(cli),
        Command::Candidate => candidate(cli),
        Command::Tag => tag(cli),
        Command::Init { version } => init(cli, version.as_ref()),
        Command::Install {
            env,
            update,
            uninstall,
        } => install(cli, env.as_deref(), *update, *uninstall),
    }
}

/// Reject the output flags on the verbs that do not report structured output.
///
/// `--format`, `--version-only` and `--output` select how a report is rendered;
/// on a verb with no report they mean nothing, so accepting them silently would
/// mislead. The reporting verbs (`status`, `release`, `candidate`, `tag`) keep
/// them.
fn reject_output_flags(cli: &Cli) -> Result<()> {
    if matches!(
        cli.command,
        Command::Status { .. } | Command::Release | Command::Candidate | Command::Tag
    ) {
        return Ok(());
    }
    for (present, flag) in [
        (cli.format.is_some(), "--format"),
        (cli.version_only, "--version-only"),
        (cli.output.is_some(), "--output"),
    ] {
        if present {
            bail!("`{flag}` is not supported by `momo {}`", cli.command);
        }
    }
    Ok(())
}

/// Record one Change in the store the configuration selects.
///
/// A write command creates `momo.toml` on first use; with `--dry-run` nothing
/// is written at all.
fn record(cli: &Cli, kind: Kind, text: &str) -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;
    let discovery = workspace::load(&root, &config)?;
    let packages = workspace::select(&discovery, cli, DefaultTarget::Cwd)?;
    let change = Change::try_new(kind, text).with_context(|| format!("recording `{kind}`"))?;

    if !cli.dry_run {
        ensure_config(&root, &config)?;
    }

    for package in packages {
        let path = recording::record(&root, &config, package, &change, cli.dry_run)?;
        if cli.dry_run {
            println!("would write {}", path.display());
        } else {
            println!("wrote {}", path.display());
        }
    }
    Ok(())
}

/// Show the Unreleased Changes and the Version they imply, per Package.
///
/// Read-only: it never writes. `check` turns it into the CI skip gate, exiting
/// non-zero when there is nothing Unreleased to release.
fn status(cli: &Cli, check: bool) -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;
    let discovery = workspace::load(&root, &config)?;
    let packages = workspace::select(&discovery, cli, DefaultTarget::All)?;
    let vcs = detect_vcs(&root);

    let mut report = output::Status {
        packages: Vec::new(),
    };
    for package in packages {
        let changes = recording::unreleased(&root, &config, package)?;
        let prefix = package.tag_prefix(&config.tag_prefix);
        let current = version::source(&package.dir(&root), &prefix, vcs.as_deref()).current()?;
        let next = release_version(&changes, &current);
        report.packages.push(StatusPackage {
            package: package.identity().to_owned(),
            current,
            next,
            changes,
        });
    }

    output::emit(cli, &report)?;

    if check && !report.unreleased() {
        bail!("nothing unreleased");
    }
    Ok(())
}

/// Mint a Release from the Unreleased Changes.
///
/// With no `-p`, every Package that has Unreleased Changes is released. Writes
/// each Package's Changelog and version file and consumes its Fragments; it
/// never commits and never tags.
fn release(cli: &Cli) -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;
    let discovery = workspace::load(&root, &config)?;
    let packages = workspace::select(&discovery, cli, DefaultTarget::All)?;
    let vcs = detect_vcs(&root);
    let explicit = cli.package.is_some();

    let mut released = 0;
    for package in packages {
        let outcome = release::release(
            &root,
            &config,
            package,
            vcs.as_deref(),
            today(),
            cli.dry_run,
        )?;
        match outcome {
            Some(outcome) => {
                report_release(cli, &outcome, Verb::Release)?;
                warn_stale_lockfile(&outcome);
                released += 1;
            }
            None if explicit => bail!("nothing unreleased to release"),
            None => {}
        }
    }
    if released == 0 {
        bail!("nothing unreleased to release");
    }
    Ok(())
}

/// Cut a Release Candidate from the Unreleased Changes.
///
/// Bumps as a Release would, then appends `-rc.N`; the Fragments are shared with
/// the eventual Release.
fn candidate(cli: &Cli) -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;
    let discovery = workspace::load(&root, &config)?;
    let packages = workspace::select(&discovery, cli, DefaultTarget::All)?;
    let vcs = detect_vcs(&root);
    let explicit = cli.package.is_some();

    let mut cut = 0;
    for package in packages {
        let outcome = release::candidate(
            &root,
            &config,
            package,
            vcs.as_deref(),
            today(),
            cli.dry_run,
        )?;
        match outcome {
            Some(outcome) => {
                report_release(cli, &outcome, Verb::Candidate)?;
                warn_stale_lockfile(&outcome);
                cut += 1;
            }
            None if explicit => bail!("nothing unreleased to release"),
            None => {}
        }
    }
    if cut == 0 {
        bail!("nothing unreleased to release");
    }
    Ok(())
}

/// Create the VCS tag for each selected Package's current Version.
fn tag(cli: &Cli) -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;
    let discovery = workspace::load(&root, &config)?;
    let packages = workspace::select(&discovery, cli, DefaultTarget::All)?;
    let Some(vcs) = detect_vcs(&root) else {
        bail!("no VCS detected — nothing to tag");
    };
    let explicit = cli.package.is_some();

    let mut tagged = 0;
    for package in packages {
        let Some(name) = release::tag(&root, &config, package, vcs.as_ref(), cli.dry_run)? else {
            if explicit {
                bail!("nothing to tag: run `momo release` first");
            }
            continue;
        };
        let version = FileVersionSource::new(&package.dir(&root)).current()?;
        output::emit(
            cli,
            &output::Tag {
                name: &name,
                version: &version,
                dry_run: cli.dry_run,
            },
        )?;
        tagged += 1;
    }
    if tagged == 0 {
        bail!("nothing to tag: run `momo release` first");
    }
    Ok(())
}

/// Report what a `release` or `candidate` did, or would do under `--dry-run`.
fn report_release(cli: &Cli, outcome: &Outcome, verb: Verb) -> Result<()> {
    output::emit(
        cli,
        &output::Release {
            version: &outcome.version,
            changelog: &outcome.changelog,
            version_file: &outcome.version_file,
            consumed: &outcome.consumed,
            verb,
            dry_run: cli.dry_run,
        },
    )
}

/// Warn that syncing a manifest left its ecosystem's lockfile stale.
///
/// The lockfile embeds the Package's version and is the package manager's to
/// refresh: momo never rewrites it and never runs the package manager.
fn warn_stale_lockfile(outcome: &Outcome) {
    let Some(update) = &outcome.manifest else {
        return;
    };
    let Some(lockfile) = &update.stale_lockfile else {
        return;
    };
    eprintln!(
        "warning: {} may be stale after syncing {}; refresh it with your package manager",
        lockfile.display(),
        update.manifest.display()
    );
}

/// The repository backing `root`, when one exists.
fn detect_vcs(root: &Path) -> Option<Box<dyn Vcs>> {
    crate::adapters::vcs::detect(root).ok()
}

/// Today's date, in UTC.
fn today() -> Date {
    time::OffsetDateTime::now_utc().date()
}

/// Scaffold `momo.toml` at the project root.
///
/// Writes the effective configuration, and with `--version <x.y.z>` records that
/// version in `.momo/version` so a fresh repo starts above the `0.0.0` bootstrap
/// baseline. With `--dry-run`, prints what would be written and leaves the tree
/// untouched. Refuses to overwrite an existing `momo.toml`.
fn init(cli: &Cli, version: Option<&Version>) -> Result<()> {
    let root = project_root()?;
    let path = root.join(CONFIG_FILE);
    let config = config::load(&root)?;

    if cli.dry_run {
        eprintln!("would write {}", path.display());
        if version.is_some() {
            eprintln!("would write {}", root.join(VERSION_FILE).display());
        }
        print!("{config}");
        return Ok(());
    }

    if path.exists() {
        bail!("{} already exists; refusing to overwrite", path.display());
    }

    fs::write(&path, config.to_string()).with_context(|| format!("writing {}", path.display()))?;
    println!("wrote {}", path.display());

    if let Some(version) = version {
        FileVersionSource::new(&root).write(version)?;
        println!("wrote {}", root.join(VERSION_FILE).display());
    }
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
        Some(name) => crate::adapters::taskenv::named(name, &root)?,
        None => match crate::adapters::taskenv::detect(&root) {
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
            Command::Status { check: false },
            Command::Release,
            Command::Candidate,
            Command::Tag,
            Command::Init { version: None },
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
