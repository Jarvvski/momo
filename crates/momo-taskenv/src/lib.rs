//! Task-environment adapters: mise in v0, implementing a port from `momo-core`.

mod detect;
mod graph;
mod mise;

pub use detect::{detect, named};
pub use graph::discover;
pub use mise::MiseTaskEnvironment;
