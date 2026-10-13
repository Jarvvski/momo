//! Task-environment adapters: mise in v0, implementing a port from
//! [`crate::domain::taskenv`].

mod detect;
mod graph;
mod mise;
mod monorepo;

pub use detect::{detect, named};
pub use graph::discover;
pub use mise::MiseTaskEnvironment;
pub use monorepo::is_monorepo;
