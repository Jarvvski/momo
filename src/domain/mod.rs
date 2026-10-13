//! momo's domain: change kinds, the bump policy, versions, the changelog and
//! config, plus the ports that adapters implement.
//!
//! This module is pure: no I/O and no CLI. Everything that touches the outside
//! world sits behind a port so it can be substituted in tests.

pub mod change;
pub mod changelog;
pub mod config;
pub mod manifest;
pub mod package;
pub mod remote;
mod slug;
pub mod taskenv;
pub mod vcs;
pub mod version;
pub mod workspace;
