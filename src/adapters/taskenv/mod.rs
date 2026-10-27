//! Task-environment adapters: mise in v0, implementing a port from
//! [`crate::domain::taskenv`].

mod detect;
mod graph;
mod mise;
mod monorepo;

pub use detect::{detect, named};
pub use graph::discover;
pub use monorepo::is_monorepo;

/// The config file names mise reads in a directory, relative to that directory.
///
/// Shared by detection ([`detect`]) and the monorepo check ([`is_monorepo`]) so
/// both recognise exactly the configurations mise would.
pub(crate) const CONFIG_FILES: [&str; 11] = [
    "mise.toml",
    "mise.local.toml",
    ".mise.toml",
    ".mise.local.toml",
    "mise/config.toml",
    "mise/config.local.toml",
    ".mise/config.toml",
    ".mise/config.local.toml",
    ".config/mise.toml",
    ".config/mise/config.toml",
    ".config/mise/config.local.toml",
];

/// The `conf.d` directories mise reads in a directory, relative to that
/// directory.
///
/// A directory holding a `*.toml` fragment in one of these is configured even
/// when no [`CONFIG_FILES`] name is present. Shared by detection ([`detect`])
/// and the monorepo check ([`is_monorepo`]) so both agree on which
/// configurations mise would read.
pub(crate) const CONF_D_DIRS: [&str; 3] = ["mise/conf.d", ".mise/conf.d", ".config/mise/conf.d"];
