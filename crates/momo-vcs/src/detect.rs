//! Repository detection: `.jj` wins over `.git`, and the nearest ancestor wins
//! over a more distant one.

use std::path::{Path, PathBuf};

use momo_core::vcs::{Vcs, VcsError, VcsKind};

use crate::git::GitVcs;
use crate::jj::JjVcs;
use crate::process::{CommandRunner, SystemRunner};

/// Detect the repository at or above `start` and build the matching adapter.
pub fn detect(start: &Path) -> Result<Box<dyn Vcs>, VcsError> {
    detect_with(SystemRunner, start)
}

/// Like [`detect`], but with an injected [`CommandRunner`].
///
/// Detection itself only reads the filesystem; the runner is threaded through so
/// the returned adapter is ready to use.
pub fn detect_with<R: CommandRunner + 'static>(
    runner: R,
    start: &Path,
) -> Result<Box<dyn Vcs>, VcsError> {
    match find_repo_root(start) {
        Some((VcsKind::Jj, root)) => Ok(Box::new(JjVcs::new(runner, root))),
        Some((VcsKind::Git, root)) => Ok(Box::new(GitVcs::new(runner, root))),
        None => Err(VcsError::NoRepository {
            path: start.to_path_buf(),
        }),
    }
}

/// The nearest ancestor of `start` that holds a `.jj` or `.git` entry, with the
/// kind that entry implies.
fn find_repo_root(start: &Path) -> Option<(VcsKind, PathBuf)> {
    let absolute = std::path::absolute(start).ok()?;
    for dir in absolute.ancestors() {
        if dir.join(".jj").is_dir() {
            return Some((VcsKind::Jj, dir.to_path_buf()));
        }
        if dir.join(".git").exists() {
            return Some((VcsKind::Git, dir.to_path_buf()));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeRunner;

    #[test]
    fn prefers_jj_when_a_directory_holds_both() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();
        std::fs::create_dir(dir.path().join(".jj")).unwrap();

        let vcs = detect_with(FakeRunner::new(), dir.path()).unwrap();
        assert_eq!(vcs.kind(), VcsKind::Jj);
    }

    #[test]
    fn finds_git_when_only_git_is_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".git")).unwrap();

        let vcs = detect_with(FakeRunner::new(), dir.path()).unwrap();
        assert_eq!(vcs.kind(), VcsKind::Git);
    }

    #[test]
    fn finds_a_repository_from_a_subdirectory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join(".jj")).unwrap();
        let nested = dir.path().join("crates/deep");
        std::fs::create_dir_all(&nested).unwrap();

        let vcs = detect_with(FakeRunner::new(), &nested).unwrap();
        assert_eq!(vcs.kind(), VcsKind::Jj);
        assert_eq!(vcs.root(), dir.path());
    }

    #[test]
    fn errors_when_no_repository_exists() {
        let dir = tempfile::tempdir().unwrap();

        let Err(err) = detect_with(FakeRunner::new(), dir.path()) else {
            panic!("expected a no-repository error");
        };
        assert!(matches!(err, VcsError::NoRepository { .. }));
    }
}
