//! Adapters: the concrete implementations of the domain's ports.
//!
//! Each submodule shells out to a tool rather than linking a library, so the
//! domain stays free of processes and the adapters are testable with a
//! scripted runner.

pub mod manifest;
pub mod spawn;
pub mod taskenv;
pub mod vcs;
