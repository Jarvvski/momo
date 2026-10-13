//! Reading mise's own configuration to tell a monorepo from a single-package
//! repo.
//!
//! `mise tasks graph --json` refuses to run unless a config in scope sets
//! `monorepo_root = true`. mise's error message is not a stable interface, so
//! momo reads the configuration instead: when no config declares a monorepo
//! root, the graph's refusal is expected and momo degrades silently.

use std::path::{Path, PathBuf};

use toml_edit::{DocumentMut, Item};

use super::CONFIG_FILES;

/// The `conf.d` directories mise reads in a directory, relative to it.
const CONF_D_DIRS: [&str; 3] = ["mise/conf.d", ".mise/conf.d", ".config/mise/conf.d"];

/// Whether the mise configuration in scope at `root` declares a monorepo root
/// (`monorepo_root = true`).
///
/// mise merges every config file from the filesystem root down to `root`, so
/// each ancestor directory is consulted; a directory's `conf.d` fragments count
/// too. Reading the configuration is deliberate: mise's error message is not a
/// stable interface.
pub fn is_monorepo(root: &Path) -> bool {
    root.ancestors().any(declares_monorepo_root)
}

/// Whether any config file directly in `dir` sets `monorepo_root = true`.
fn declares_monorepo_root(dir: &Path) -> bool {
    CONFIG_FILES
        .iter()
        .map(|name| dir.join(name))
        .any(|path| file_declares_monorepo_root(&path))
        || CONF_D_DIRS
            .iter()
            .map(|name| dir.join(name))
            .any(|path| conf_d_declares_monorepo_root(&path))
}

/// Whether the TOML file at `path` sets `monorepo_root = true`.
///
/// A missing, unreadable, or invalid file simply does not declare a monorepo
/// root.
fn file_declares_monorepo_root(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .is_some_and(|text| text_declares_monorepo_root(&text))
}

/// Whether any `*.toml` fragment in the `conf.d` directory `dir` sets
/// `monorepo_root = true`.
fn conf_d_declares_monorepo_root(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.filter_map(Result::ok).any(|entry| {
        let path: PathBuf = entry.path();
        path.extension()
            .is_some_and(|extension| extension == "toml")
            && file_declares_monorepo_root(&path)
    })
}

/// Whether `text` is TOML with a top-level `monorepo_root = true`.
fn text_declares_monorepo_root(text: &str) -> bool {
    text.parse::<DocumentMut>()
        .ok()
        .and_then(|document| document.get("monorepo_root").and_then(Item::as_bool))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::TempDir;

    fn write(dir: &TempDir, path: &str, contents: &str) {
        let path = dir.path().join(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn a_mise_toml_with_monorepo_root_is_a_monorepo() {
        let dir = TempDir::new().unwrap();
        write(&dir, "mise.toml", "monorepo_root = true\n");
        assert!(is_monorepo(dir.path()));
    }

    #[test]
    fn a_mise_toml_without_monorepo_root_is_not_a_monorepo() {
        let dir = TempDir::new().unwrap();
        write(&dir, "mise.toml", "[tools]\nrust = \"stable\"\n");
        assert!(!is_monorepo(dir.path()));
    }

    #[test]
    fn monorepo_root_false_is_not_a_monorepo() {
        let dir = TempDir::new().unwrap();
        write(&dir, "mise.toml", "monorepo_root = false\n");
        assert!(!is_monorepo(dir.path()));
    }

    #[test]
    fn a_dot_mise_config_is_read() {
        let dir = TempDir::new().unwrap();
        write(&dir, ".mise/config.toml", "monorepo_root = true\n");
        assert!(is_monorepo(dir.path()));
    }

    #[test]
    fn an_ancestor_config_counts() {
        let dir = TempDir::new().unwrap();
        write(&dir, "mise.toml", "monorepo_root = true\n");
        let nested = dir.path().join("crates/a");
        std::fs::create_dir_all(&nested).unwrap();
        assert!(is_monorepo(&nested));
    }

    #[test]
    fn no_config_is_not_a_monorepo() {
        let dir = TempDir::new().unwrap();
        assert!(!is_monorepo(dir.path()));
    }

    #[test]
    fn invalid_toml_is_not_a_monorepo() {
        let dir = TempDir::new().unwrap();
        write(&dir, "mise.toml", "monorepo_root = \n");
        assert!(!is_monorepo(dir.path()));
    }
}
