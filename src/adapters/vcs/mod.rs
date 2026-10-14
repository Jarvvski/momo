//! Version-control adapters: git and jj, each implementing a port from
//! [`crate::domain::vcs`].
//!
//! Both shell out to their CLI through [`CommandRunner`] — no VCS library — so
//! the adapters are testable with a scripted runner and the domain stays free of
//! processes.

mod detect;
mod git;
mod jj;
mod process;
mod tags;

#[cfg(test)]
mod roundtrip;
#[cfg(test)]
mod test_support;

pub use detect::{detect, detect_with};
pub use git::GitVcs;
pub use jj::JjVcs;
pub use process::{CommandRunner, SystemRunner};
// The process vocabulary lives in `spawn` (shared with the taskenv adapter);
// re-exported here so the vcs public surface keeps naming its runner's output.
pub use crate::adapters::spawn::{ProcessError, ProcessOutput};
