//! Command handlers.
//!
//! Every verb on the command surface has a handler: the change verbs
//! (`break`/`add`/`fix`) record a Change, `status` reports the Unreleased
//! Changes and the Version they imply, `release`/`candidate` mint one, `tag`
//! creates the VCS tag, `init`/`install` scaffold config and task wrappers, and
//! `version` prints the CLI version.
//!
//! Package-aware verbs resolve the Packages they act on through
//! [`crate::workspace`]: `-p` scopes to one, `--all` covers every one, and the
//! cwd-relative default applies otherwise. Each Package is reported through
//! [`crate::output`].

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::domain::taskenv::{InstallMode, InstallPlan, TaskEnvironment};
use crate::domain::vcs::Vcs;
use crate::domain::workspace::DefaultTarget;
use anyhow::{Context, Result, bail};
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use semver::Version;
use time::Date;

use crate::domain::change::{Change, Kind};
use crate::domain::config::{Config, Recording};
use crate::domain::package::Package;
use crate::domain::tag;
use crate::domain::version::next_version;

use crate::cli::config::{self, CONFIG_FILE};
use crate::cli::output::{self, ReleasePackage, SkippedManifest, StatusPackage, TagPackage, Verb};
use crate::cli::recording;
use crate::cli::release::{self, Outcome};
use crate::cli::version::{self, FileVersionSource, VERSION_FILE};
use crate::cli::workspace;
use crate::cli::{Cli, Command};

/// The momo verbs exposed as `momo:*` tasks, in command-surface order.
///
/// The command surface is wider than this list: `version` and `completions`
/// report and write nothing, so neither must ever become a task wrapper.
const INSTALLABLE_VERBS: [&str; 9] = [
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
        Command::Tag { rev } => tag(cli, rev.as_deref()),
        Command::Init { version } => init(cli, version.as_ref()),
        Command::Install {
            env,
            update,
            uninstall,
        } => install(cli, env.as_deref(), *update, *uninstall),
        Command::Version => show_version(),
        Command::Completions { shell } => completions(*shell),
    }
}

/// Reject the output flags on the verbs that do not report structured output.
///
/// `--format`, its `--json` shorthand, `--version-only` and `--output` select
/// how a report is rendered; on a verb with no report they mean nothing, so
/// accepting them silently would mislead. The reporting verbs (`status`,
/// `release`, `candidate`, `tag`) keep them.
fn reject_output_flags(cli: &Cli) -> Result<()> {
    if matches!(
        cli.command,
        Command::Status { .. } | Command::Release | Command::Candidate | Command::Tag { .. }
    ) {
        return Ok(());
    }
    for (present, flag) in [
        (cli.format.is_some(), "--format"),
        (cli.json, "--json"),
        (cli.version_only, "--version-only"),
        (cli.output.is_some(), "--output"),
    ] {
        if present {
            bail!("`{flag}` is not supported by `momo {}`", cli.command);
        }
    }
    Ok(())
}

/// Print the momo version, exactly as `--version` does.
fn show_version() -> Result<()> {
    println!("momo {VERSION}");
    Ok(())
}

/// Write a shell completion script to stdout.
///
/// Generated from the same clap tree the parser uses, so the script cannot
/// drift from the command surface. Reads nothing and writes nothing on disk:
/// redirect the output where the shell expects to find it.
fn completions(shell: Shell) -> Result<()> {
    let mut command = Cli::command();
    let mut stdout = io::stdout();
    generate(shell, &mut command, "momo", &mut stdout);
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
        ensure_config(&root)?;
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
    let vcs = detect_vcs(&root, &config);

    let mut report = output::Status {
        packages: Vec::new(),
    };
    for package in packages {
        let changes = recording::unreleased(&root, &config, package)?;
        let prefix = tag::prefix(package, &config.tag_prefix);
        let changelog = release::read_changelog(&root, &config, package)?;
        // Only changelog mode reads the Unreleased body for the bump, so only
        // there can a heading momo cannot read hide Unreleased Changes. In
        // fragments mode the changelog's Unreleased is a separate error.
        let unversioned = match config.recording {
            Recording::Changelog => changelog.unread_sections(),
            Recording::Fragments => Vec::new(),
        };
        let source = version::source(&package.dir(&root), &prefix, vcs.as_deref());
        let base = version::release_base(source.as_ref(), &changelog)?;
        let current = source.current()?;
        let next = next_version(&changes, &base)?;
        report.packages.push(StatusPackage {
            package: package.identity().to_owned(),
            current,
            next,
            changes,
            unversioned,
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
    let vcs = detect_vcs(&root, &config);

    // Read the date once for the whole run: `today()` inside the loop could
    // straddle UTC midnight and stamp a multi-package release with two dates.
    let date = today();

    // Resolve every Package before writing any of them: a shared workspace
    // version is validated across the whole run, not per Package.
    let mut planned = Vec::new();
    for package in packages {
        if let Some(prepared) =
            release::prepare_release(&root, &config, package, vcs.as_deref(), date)?
        {
            planned.push((package, prepared));
        }
    }
    release::reject_shared_manifest_conflicts(&planned)?;
    for (package, prepared) in &planned {
        warn_go_major(package, prepared.version());
    }

    let mut report = output::Release {
        verb: Verb::Release,
        dry_run: cli.dry_run,
        packages: Vec::new(),
    };
    let mut committed = Vec::new();
    for (package, prepared) in planned {
        if let Some(outcome) = release::commit(prepared, cli.dry_run)? {
            committed.push((package, outcome));
        }
    }
    report_manifest(committed.iter().map(|(_, outcome)| outcome));
    for (package, outcome) in committed {
        report
            .packages
            .push(release_package(package.identity(), outcome));
    }
    if report.packages.is_empty() {
        bail!("nothing unreleased to release");
    }

    // One report, one `emit`: emitting per Package would concatenate N JSON
    // documents and truncate `--output` to the last Package.
    output::emit(cli, &report)
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
    let vcs = detect_vcs(&root, &config);

    // Read the date once for the whole run: `today()` inside the loop could
    // straddle UTC midnight and stamp a multi-package release with two dates.
    let date = today();

    // Resolve every Package before writing any of them: a shared workspace
    // version is validated across the whole run, not per Package.
    let mut planned = Vec::new();
    for package in packages {
        if let Some(prepared) =
            release::prepare_candidate(&root, &config, package, vcs.as_deref(), date)?
        {
            planned.push((package, prepared));
        }
    }
    release::reject_shared_manifest_conflicts(&planned)?;
    for (package, prepared) in &planned {
        warn_go_major(package, prepared.version());
    }

    let mut report = output::Release {
        verb: Verb::Candidate,
        dry_run: cli.dry_run,
        packages: Vec::new(),
    };
    let mut committed = Vec::new();
    for (package, prepared) in planned {
        if let Some(outcome) = release::commit(prepared, cli.dry_run)? {
            committed.push((package, outcome));
        }
    }
    report_manifest(committed.iter().map(|(_, outcome)| outcome));
    for (package, outcome) in committed {
        report
            .packages
            .push(release_package(package.identity(), outcome));
    }
    if report.packages.is_empty() {
        bail!("nothing unreleased to release");
    }
    output::emit(cli, &report)
}

/// Create the VCS tag for each selected Package's current Version.
///
/// `rev` overrides the adapter's default anchor: an uncommitted release passes
/// `--rev @` on jj, where the default is the committed release at `@-`.
fn tag(cli: &Cli, rev: Option<&str>) -> Result<()> {
    let root = project_root()?;
    let config = config::load(&root)?;
    let discovery = workspace::load(&root, &config)?;
    let packages = workspace::select(&discovery, cli, DefaultTarget::All)?;
    let Some(vcs) = detect_vcs(&root, &config) else {
        bail!("no VCS detected — nothing to tag");
    };
    let explicit = cli.package.is_some();

    let mut report = output::Tag {
        dry_run: cli.dry_run,
        packages: Vec::new(),
    };
    for package in packages {
        let Some(tagged_package) =
            release::tag(&root, &config, package, vcs.as_ref(), rev, cli.dry_run)?
        else {
            if explicit {
                bail!("nothing to tag: run `momo release` first");
            }
            continue;
        };
        report.packages.push(TagPackage {
            package: package.identity().to_owned(),
            name: tagged_package.name,
            version: tagged_package.version,
        });
    }
    if report.packages.is_empty() {
        bail!("nothing to tag: run `momo release` first");
    }
    output::emit(cli, &report)
}

/// One Package's entry in a `release`/`candidate` report.
fn release_package(package: &str, outcome: Outcome) -> ReleasePackage {
    let Outcome {
        version,
        changelog,
        version_file,
        consumed,
        manifest,
    } = outcome;
    let (manifest, skipped) = match manifest {
        Some(plan) => (
            plan.writes.into_iter().map(|write| write.path).collect(),
            plan.skipped
                .into_iter()
                .map(|(path, reason)| SkippedManifest {
                    path,
                    reason: reason.to_string(),
                })
                .collect(),
        ),
        None => (Vec::new(), Vec::new()),
    };

    ReleasePackage {
        package: package.to_owned(),
        version,
        changelog,
        version_file,
        manifest,
        skipped,
        consumed,
    }
}

/// Warn when a Package's minted Version cannot resolve as a proper Go major.
///
/// From major 2 onward Go serves a version only when the module path ends in
/// `/vN`: a `go:` module whose path lacks the suffix is served as
/// `+incompatible`. The warning names that consequence; it never blocks the
/// release.
fn warn_go_major(package: &Package, version: &Version) {
    if let Some(module) = tag::go_module_path(package)
        && let Some(warning) = tag::go_major_warning(module, version)
    {
        eprintln!("warning: {warning}");
    }
}

/// Report what manifest sync skipped and what it left stale, once for the run.
fn report_manifest<'a>(outcomes: impl Iterator<Item = &'a Outcome>) {
    for line in manifest_warnings(outcomes) {
        eprintln!("{line}");
    }
}

/// The warnings [`report_manifest`] prints, in order.
///
/// A skip is never silent: a manifest momo considered but did not write — no
/// version scalar, or one computed elsewhere — is named with its reason. A
/// stale lockfile is the package manager's to refresh, and is named once however
/// many Packages resolve to it: a workspace's members share one root lockfile,
/// so warning per Package would only repeat it.
fn manifest_warnings<'a>(outcomes: impl Iterator<Item = &'a Outcome>) -> Vec<String> {
    let mut lines = Vec::new();
    let mut reported = BTreeSet::new();
    for outcome in outcomes {
        let Some(plan) = &outcome.manifest else {
            continue;
        };
        for (path, reason) in &plan.skipped {
            lines.push(format!("skipped {}: {reason}", path.display()));
        }
        for lockfile in &plan.lockfiles {
            if reported.insert(lockfile.as_path()) {
                lines.push(format!(
                    "warning: {} may be stale after syncing a manifest; {}",
                    lockfile.display(),
                    refresh_command(lockfile)
                ));
            }
        }
    }
    lines
}

/// How to refresh `lockfile` after a manifest sync, as the warning tells it.
///
/// The command is a hint momo never runs — refreshing a lockfile is the package
/// manager's job. A lockfile momo does not recognise falls back to naming the
/// package manager generically.
fn refresh_command(lockfile: &Path) -> String {
    match lockfile.file_name().and_then(|name| name.to_str()) {
        Some("package-lock.json") => "refresh it with `npm install --package-lock-only`".to_owned(),
        Some("pnpm-lock.yaml") => "refresh it with `pnpm install --lockfile-only`".to_owned(),
        Some("yarn.lock") => "refresh it with `yarn install --mode update-lockfile`".to_owned(),
        Some("bun.lockb" | "bun.lock") => {
            "refresh it with `bun install --lockfile-only`".to_owned()
        }
        _ => "refresh it with your package manager".to_owned(),
    }
}

/// The repository backing `root`, when one exists.
///
/// The adapter's spawned commands are bounded by the configured timeout, so a
/// hung git or jj cannot stall the run.
fn detect_vcs(root: &Path, config: &Config) -> Option<Box<dyn Vcs>> {
    crate::adapters::vcs::detect_with(
        crate::adapters::vcs::SystemRunner::new(config.command_timeout),
        root,
    )
    .ok()
}

/// Today's date, in UTC.
///
/// UTC is deliberate: a changelog date must be reproducible across machines and
/// CI runners, whatever the local timezone. Callers read it once per run so a
/// multi-package release cannot straddle midnight and stamp two dates.
fn today() -> Date {
    time::OffsetDateTime::now_utc().date()
}

/// Scaffold `momo.toml` at the project root.
///
/// Writes the default configuration - never the `MOMO_*` environment, which is
/// transient and must not be baked into the committed file - and with
/// `--version <x.y.z>` records that version in `.momo/version` so a fresh repo
/// starts above the `0.0.0` bootstrap baseline. With `--dry-run`, prints what
/// would be written and leaves the tree untouched. Refuses to overwrite an
/// existing `momo.toml`.
fn init(cli: &Cli, version: Option<&Version>) -> Result<()> {
    let root = project_root()?;
    let path = root.join(CONFIG_FILE);
    let config = Config::default();

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
/// Uses the explicit `env` override, else the environment recorded in
/// `momo.toml` by a previous install, else auto-detection of mise at or above
/// the project root. With none of these this is a warning and a no-op. `update`
/// refreshes the pinned version; `uninstall` removes what install wrote.
fn install(cli: &Cli, env: Option<&str>, update: bool, uninstall: bool) -> Result<()> {
    let cwd = env::current_dir().context("resolving the current directory")?;
    let root = config::project_root(&cwd);

    let environment = match env {
        Some(name) => crate::adapters::taskenv::named(name, &root)?,
        None => {
            let recorded = config::load(&root)?.task_env;
            match recorded.as_deref() {
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
            }
        }
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
    let plan = environment.plan(&version, &INSTALLABLE_VERBS, mode)?;

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

/// Create `momo.toml` with the defaults on first write, so the config is
/// optional.
///
/// Only the default layer is persisted: an environment override is transient,
/// and baking it into the committed file would invert the documented precedence.
fn ensure_config(root: &Path) -> Result<()> {
    let path = root.join(CONFIG_FILE);
    if path.exists() {
        return Ok(());
    }
    let config = Config::default();
    fs::write(&path, config.to_string()).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::domain::manifest::{ManifestPlan, SkipReason};

    #[test]
    fn every_installable_command_has_a_task_wrapper() {
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
            Command::Tag { rev: None },
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
                INSTALLABLE_VERBS.contains(&verb.as_str()),
                "no task wrapper for `{verb}`"
            );
        }
        assert_eq!(
            INSTALLABLE_VERBS,
            [
                "break",
                "add",
                "fix",
                "status",
                "release",
                "candidate",
                "tag",
                "init",
                "install",
            ],
            "the verb list drifted from the surface"
        );
    }

    /// An `Outcome` carrying `plan` and nothing else that matters here.
    fn outcome(manifest: Option<ManifestPlan>) -> Outcome {
        Outcome {
            version: Version::new(0, 1, 0),
            changelog: PathBuf::from("CHANGELOG.md"),
            version_file: PathBuf::from(".momo/version"),
            consumed: Vec::new(),
            manifest,
        }
    }

    /// A plan that left `lockfile` stale.
    fn stale(lockfile: &str) -> ManifestPlan {
        ManifestPlan {
            writes: Vec::new(),
            lockfiles: vec![PathBuf::from(lockfile)],
            skipped: Vec::new(),
        }
    }

    #[test]
    fn a_shared_lockfile_is_reported_once() {
        // Two members of one workspace resolve to the same root lockfile.
        let a = outcome(Some(stale("/repo/Cargo.lock")));
        let b = outcome(Some(stale("/repo/Cargo.lock")));

        let lines = manifest_warnings([&a, &b].into_iter());

        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("Cargo.lock"), "{lines:?}");
        assert!(lines[0].contains("may be stale"), "{lines:?}");
    }

    #[test]
    fn a_node_lockfile_warns_with_its_refresh_command() {
        let a = outcome(Some(stale("/repo/pnpm-lock.yaml")));

        let lines = manifest_warnings(std::iter::once(&a));

        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(
            lines[0].contains("pnpm install --lockfile-only"),
            "{lines:?}"
        );
    }

    #[test]
    fn an_unknown_lockfile_falls_back_to_a_generic_warning() {
        assert_eq!(
            refresh_command(Path::new("/repo/mystery.lock")),
            "refresh it with your package manager"
        );
    }

    #[test]
    fn a_skip_is_reported_with_its_reason() {
        let skipped = outcome(Some(ManifestPlan {
            writes: Vec::new(),
            lockfiles: Vec::new(),
            skipped: vec![(PathBuf::from("Cargo.toml"), SkipReason::NoVersion)],
        }));

        assert_eq!(
            manifest_warnings(std::iter::once(&skipped)),
            vec!["skipped Cargo.toml: no version momo owns"]
        );
    }

    #[test]
    fn a_package_without_a_manifest_reports_nothing() {
        assert!(manifest_warnings(std::iter::once(&outcome(None))).is_empty());
    }
}
