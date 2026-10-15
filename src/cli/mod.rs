//! The complete momo command-line surface.
//!
//! Every verb, alias, and global flag is declared here up front so later work
//! fills in a handler rather than adding surface. Verbs without a handler yet
//! exit non-zero with a clear message (see [`crate::cli::commands`]).

pub(crate) mod atomic;
pub(crate) mod cascade;
pub(crate) mod commands;
pub(crate) mod config;
pub(crate) mod output;
pub(crate) mod recording;
pub(crate) mod release;
pub(crate) mod version;
pub(crate) mod workspace;

use std::fmt;
use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
use semver::Version;

/// The quick-start block `momo --help` closes with.
const CLI_EXAMPLES: &str = "\
Examples:
  momo add \"Add a shiny thing\"    record a change (break | add | fix)
  momo fix \"Fix a nasty bug\"
  momo status                      show what would be released, and at which version
  momo release                     mint the version and write the changelog
  momo tag                         tag the release you just committed
  momo init                        scaffold momo.toml (optional; commands create it lazily)

Run `momo <verb> --help` for a verb's own examples.";

/// The per-verb usage examples, shown by `momo <verb> --help`.
const BREAK_EXAMPLES: &str = "Examples:\n  momo break \"Drop support for Node 18\"";
const ADD_EXAMPLES: &str = "Examples:\n  momo add \"Add a --json output shorthand\"";
const FIX_EXAMPLES: &str = "Examples:\n  momo fix \"Fix a panic on an empty changelog\"";
const STATUS_EXAMPLES: &str = "\
Examples:
  momo status
  momo status --check    exit non-zero when there is nothing to release";
const RELEASE_EXAMPLES: &str = "\
Examples:
  momo release
  momo release --version-only    print the minted version, for CI";
const CANDIDATE_EXAMPLES: &str = "Examples:\n  momo candidate    cut 1.3.0-rc.1, then 1.3.0-rc.2";
const TAG_EXAMPLES: &str = "\
Examples:
  momo tag
  momo tag --rev @    tag the working copy (jj)";
const INIT_EXAMPLES: &str = "\
Examples:
  momo init
  momo init --version 1.2.3";
const INSTALL_EXAMPLES: &str = "\
Examples:
  momo install
  momo install --update    refresh the pinned CLI version";
const VERSION_EXAMPLES: &str = "Examples:\n  momo version";
const COMPLETIONS_EXAMPLES: &str = "\
Examples:
  momo completions bash > ~/.local/share/bash-completion/completions/momo
  momo completions zsh > ~/.zsh/completions/_momo";

/// The parsed command line.
#[derive(Debug, Parser)]
#[command(
    name = "momo",
    version,
    about = "Turn a project's changes into semver releases, a changelog, and VCS tags",
    after_help = CLI_EXAMPLES
)]
pub struct Cli {
    /// The command to run.
    #[command(subcommand)]
    pub command: Command,

    /// Show what would happen and change nothing.
    #[arg(long, global = true)]
    pub dry_run: bool,

    /// Output format for commands that report structured data.
    #[arg(long, global = true, value_enum)]
    pub format: Option<Format>,

    /// Shorthand for `--format json`.
    #[arg(long, global = true, conflicts_with = "format")]
    pub json: bool,

    /// Print only the computed version.
    #[arg(long, global = true)]
    pub version_only: bool,

    /// Write output to a file instead of stdout.
    #[arg(long, global = true, value_name = "PATH")]
    pub output: Option<PathBuf>,

    /// Target a package (default: the one containing the current directory).
    #[arg(short = 'p', long = "package", global = true, value_name = "PKG")]
    pub package: Option<String>,

    /// Operate on every package.
    #[arg(long, global = true)]
    pub all: bool,
}

impl Cli {
    /// The output format the global flags select.
    ///
    /// `--json` is the shorthand for `--format json`; clap rejects the two
    /// together, so only one of them can be set.
    pub fn output_format(&self) -> Format {
        if self.json {
            Format::Json
        } else {
            self.format.unwrap_or_default()
        }
    }
}

/// The commands momo exposes.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Record a breaking change.
    #[command(visible_alias = "b", after_help = BREAK_EXAMPLES)]
    Break {
        /// The change description.
        text: String,
    },

    /// Record an added change.
    #[command(visible_alias = "a", after_help = ADD_EXAMPLES)]
    Add {
        /// The change description.
        text: String,
    },

    /// Record a fixed change.
    #[command(visible_alias = "f", after_help = FIX_EXAMPLES)]
    Fix {
        /// The change description.
        text: String,
    },

    /// Show unreleased changes and the version they would mint.
    #[command(after_help = STATUS_EXAMPLES)]
    Status {
        /// Exit non-zero when there is nothing Unreleased (the CI skip gate).
        #[arg(long)]
        check: bool,
    },

    /// Mint a version from the unreleased changes.
    #[command(after_help = RELEASE_EXAMPLES)]
    Release,

    /// Cut or increment a release candidate.
    #[command(visible_alias = "rc", after_help = CANDIDATE_EXAMPLES)]
    Candidate,

    /// Create the VCS tag for the current version.
    #[command(after_help = TAG_EXAMPLES)]
    Tag {
        /// Tag this revision instead of the default anchor (jj: `@-`, git: `HEAD`).
        #[arg(long, value_name = "REV")]
        rev: Option<String>,
    },

    /// Scaffold momo.toml.
    #[command(after_help = INIT_EXAMPLES)]
    Init {
        /// A semver version to record in `.momo/version`, so a fresh repo starts
        /// above the `0.0.0` bootstrap baseline.
        #[arg(long, value_name = "X.Y.Z")]
        version: Option<Version>,
    },

    /// Install momo into a task environment.
    #[command(after_help = INSTALL_EXAMPLES)]
    Install {
        /// The task environment to install into (auto-detected when omitted).
        env: Option<String>,

        /// Refresh the pinned CLI version of an existing install.
        #[arg(long, conflicts_with = "uninstall")]
        update: bool,

        /// Remove what a previous install wrote.
        #[arg(long)]
        uninstall: bool,
    },

    /// Print the momo version.
    #[command(after_help = VERSION_EXAMPLES)]
    Version,

    /// Write a shell completion script to stdout.
    #[command(after_help = COMPLETIONS_EXAMPLES)]
    Completions {
        /// The shell to generate completions for.
        #[arg(value_enum)]
        shell: Shell,
    },
}

impl fmt::Display for Command {
    /// The canonical verb for the command.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Break { .. } => "break",
            Self::Add { .. } => "add",
            Self::Fix { .. } => "fix",
            Self::Status { .. } => "status",
            Self::Release => "release",
            Self::Candidate => "candidate",
            Self::Tag { .. } => "tag",
            Self::Init { .. } => "init",
            Self::Install { .. } => "install",
            Self::Version => "version",
            Self::Completions { .. } => "completions",
        })
    }
}

/// The output format for commands that report structured data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum Format {
    /// Human-readable text.
    #[default]
    Human,
    /// Machine-readable JSON.
    Json,
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Human => "human",
            Self::Json => "json",
        })
    }
}
