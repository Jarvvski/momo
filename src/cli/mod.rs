//! The complete momo command-line surface.
//!
//! Every verb, alias, and global flag is declared here up front so later work
//! fills in a handler rather than adding surface. Verbs without a handler yet
//! exit non-zero with a clear message (see [`crate::cli::commands`]).

pub(crate) mod atomic;
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
use semver::Version;

/// The parsed command line.
#[derive(Debug, Parser)]
#[command(
    name = "momo",
    version,
    about = "Turn a project's changes into semver releases, a changelog, and VCS tags"
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

/// The commands momo exposes.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Record a breaking change.
    #[command(visible_alias = "b")]
    Break {
        /// The change description.
        text: String,
    },

    /// Record an added change.
    #[command(visible_alias = "a")]
    Add {
        /// The change description.
        text: String,
    },

    /// Record a fixed change.
    #[command(visible_alias = "f")]
    Fix {
        /// The change description.
        text: String,
    },

    /// Show unreleased changes and the version they would mint.
    Status {
        /// Exit non-zero when there is nothing Unreleased (the CI skip gate).
        #[arg(long)]
        check: bool,
    },

    /// Mint a version from the unreleased changes.
    Release,

    /// Cut or increment a release candidate.
    #[command(visible_alias = "rc")]
    Candidate,

    /// Create the VCS tag for the current version.
    Tag {
        /// Tag this revision instead of the default anchor (jj: `@-`, git: `HEAD`).
        #[arg(long, value_name = "REV")]
        rev: Option<String>,
    },

    /// Scaffold momo.toml.
    Init {
        /// A semver version to record in `.momo/version`, so a fresh repo starts
        /// above the `0.0.0` bootstrap baseline.
        #[arg(long, value_name = "X.Y.Z")]
        version: Option<Version>,
    },

    /// Install momo into a task environment.
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
