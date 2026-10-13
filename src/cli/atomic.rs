//! Atomic file replacement: a temporary file beside the target, then a rename.
//!
//! Recording and the Release transaction both need to replace a file without a
//! reader — or a crash mid-write — ever seeing it half-written. The pattern is
//! the same everywhere: render the whole contents into a temporary file in the
//! target's own directory (so the rename cannot cross a filesystem), then rename
//! it over the target, which is atomic on POSIX.

use std::fs::{self, File, OpenOptions};
use std::io::{self, ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Makes each temporary file name unique within the process.
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Replace `target` with `contents`, atomically.
///
/// A reader sees either the whole old file or the whole new one, never a torn
/// write, and a failure leaves `target` exactly as it was.
pub fn write(target: &Path, contents: &str) -> io::Result<()> {
    let temp = stage(target, contents)?;
    match fs::rename(&temp, target) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(&temp);
            Err(err)
        }
    }
}

/// Write `contents` to a temporary file beside `target`, returning its path.
///
/// The caller renames it over `target` (as [`write`] does) or removes it. The
/// temporary is created with `create_new`, so two writers can never share one.
pub fn stage(target: &Path, contents: &str) -> io::Result<PathBuf> {
    let (temp, mut file) = create(target)?;
    if let Err(err) = file.write_all(contents.as_bytes()) {
        drop(file);
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    Ok(temp)
}

/// Create a fresh temporary file for `target`, retrying on a name collision.
fn create(target: &Path) -> io::Result<(PathBuf, File)> {
    loop {
        let temp = temp_path(target);
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
}

/// A unique temporary path in `target`'s own directory.
fn temp_path(target: &Path) -> PathBuf {
    let stem = target
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let name = format!(
        ".{stem}.momo-{}-{}.tmp",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    match target
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        Some(parent) => parent.join(name),
        None => PathBuf::from(name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    /// The entries directly under `dir`, sorted.
    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .expect("read the directory")
            .map(|entry| {
                entry
                    .expect("a directory entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        names.sort();
        names
    }

    #[test]
    fn write_replaces_the_contents_and_leaves_no_temporary_behind() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("file.txt");
        fs::write(&path, "old\n").expect("seed the file");

        write(&path, "new\n").expect("write atomically");

        assert_eq!(fs::read_to_string(&path).expect("read"), "new\n");
        assert_eq!(entries(dir.path()), ["file.txt"]);
    }

    #[test]
    fn write_creates_a_missing_file() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("file.txt");

        write(&path, "new\n").expect("write atomically");

        assert_eq!(fs::read_to_string(&path).expect("read"), "new\n");
    }

    #[test]
    fn stage_writes_beside_the_target_and_leaves_the_target_alone() {
        let dir = TempDir::new().expect("temp dir");
        let path = dir.path().join("file.txt");
        fs::write(&path, "old\n").expect("seed the file");

        let temp = stage(&path, "new\n").expect("stage");

        assert_eq!(temp.parent(), Some(dir.path()));
        assert_eq!(fs::read_to_string(&temp).expect("read the temp"), "new\n");
        assert_eq!(fs::read_to_string(&path).expect("read the target"), "old\n");
    }

    #[test]
    fn a_target_with_no_parent_stages_in_the_current_directory() {
        let temp = temp_path(Path::new("Cargo.toml"));

        assert_eq!(temp.parent(), Some(Path::new("")));
        assert!(
            temp.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(".Cargo.toml.momo-")),
            "{temp:?}"
        );
    }
}
