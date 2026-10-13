//! momo's domain: change kinds, the bump policy, versions, the changelog and
//! config, plus the ports that adapters implement.
//!
//! This crate is pure: no I/O and no CLI. Everything that touches the outside
//! world sits behind a port so it can be substituted in tests.

pub mod config;
pub mod taskenv;
pub mod vcs;
