//! The domain purity guard, enforced by `cargo test`.
//!
//! `src/domain` is the pure core: it never touches the filesystem, spawns a
//! process, reads the environment, opens a socket, or depends on `clap` or
//! `anyhow`. `mise run lint:domain` greps for the obvious forms as a fast
//! pre-flight, but a grep a contributor can skip is not enforcement — this test
//! is the source of truth, because `mise run check` runs `cargo test`.
//!
//! `std::io` is deliberately *not* banned: the domain's file-backed error types
//! still carry `std::io::Error`. Ticket 39 reshaped only the *process*
//! vocabulary (the binary, its arguments, its stderr) into an opaque source, so
//! `io` stays out of scope; add it here (and in `mise.toml`) if a later ticket
//! boxes those too.

use std::fs;
use std::path::{Path, PathBuf};

/// The `std` modules the domain must not reach for.
const FORBIDDEN_MODULES: [&str; 4] = ["fs", "process", "env", "net"];

/// The crates the domain must not depend on.
const FORBIDDEN_CRATES: [&str; 2] = ["clap", "anyhow"];

/// The pure core, located from the crate root.
fn domain_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src/domain")
}

/// Every `.rs` file under `dir`, recursively, in a stable order.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries =
            fs::read_dir(&dir).unwrap_or_else(|err| panic!("reading {}: {err}", dir.display()));
        for entry in entries {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

/// Whether `text` uses `module::` as a path segment, bare or behind `std::`.
///
/// The character before the module must not be an identifier character, so
/// `taskenv::` does not read as a bare `env::`.
fn uses_module_path(text: &str, module: &str) -> bool {
    let needle = format!("{module}::");
    text.match_indices(needle.as_str()).any(|(at, _)| {
        text[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !is_ident(before))
    })
}

/// Whether `text` names `token` as a whole word.
fn names_token(text: &str, token: &str) -> bool {
    text.match_indices(token).any(|(at, _)| {
        let end = at + token.len();
        text[..at]
            .chars()
            .next_back()
            .is_none_or(|before| !is_ident(before))
            && text[end..]
                .chars()
                .next()
                .is_none_or(|after| !is_ident(after))
    })
}

/// Whether `ch` can appear inside an identifier.
fn is_ident(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// Every `use std::…;` statement, as a 1-based starting line and the whole
/// statement, continuation lines joined.
///
/// A grouped import (`use std::{env, fs};`) names no `env::`/`fs::` path, and a
/// wrapped one puts the modules on later lines, so the imports are collected
/// whole rather than matched line by line.
fn std_imports(source: &str) -> Vec<(usize, String)> {
    let mut imports = Vec::new();
    let mut lines = source.lines().enumerate();
    while let Some((index, line)) = lines.next() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("use std::") || trimmed.starts_with("pub use std::")) {
            continue;
        }
        let mut statement = line.to_owned();
        while !statement.contains(';') {
            let Some((_, next)) = lines.next() else {
                break;
            };
            statement.push('\n');
            statement.push_str(next);
        }
        imports.push((index + 1, statement));
    }
    imports
}

/// Every purity violation in `source`, as a 1-based line and an explanation.
///
/// Catches `std::fs`-style paths, bare `env::var`/`fs::read`, grouped
/// `use std::{env, fs};` imports (wrapped or not), and `clap`/`anyhow`.
fn violations(source: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        for module in FORBIDDEN_MODULES {
            if uses_module_path(line, module) {
                found.push((line_number, format!("uses `{module}::`")));
            }
        }
        for krate in FORBIDDEN_CRATES {
            if names_token(line, krate) {
                found.push((line_number, format!("depends on `{krate}`")));
            }
        }
    }

    for (line, statement) in std_imports(source) {
        for module in FORBIDDEN_MODULES {
            if names_token(&statement, module) {
                found.push((line, format!("imports `std::{module}`")));
            }
        }
    }

    found
}

#[test]
fn the_domain_stays_pure() {
    let dir = domain_dir();
    let files = rust_files(&dir);
    assert!(
        !files.is_empty(),
        "no Rust sources found under {} — the walk is broken",
        dir.display()
    );

    let mut found = Vec::new();
    for path in files {
        let source = fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("reading {}: {err}", path.display()));
        for (line, why) in violations(&source) {
            found.push(format!("{}:{line}: {why}", path.display()));
        }
    }

    assert!(
        found.is_empty(),
        "src/domain must stay pure:\n{}",
        found.join("\n")
    );
}

#[test]
fn the_checker_flags_the_forms_a_grep_misses() {
    for snippet in [
        "use std::{env, fs};",
        "use std::env;",
        "use std::process::Command;",
        "fn f() { let _ = std::process::Command::new(\"x\"); }",
        "fn f() { let _ = env::var(\"HOME\"); }",
        "fn f() { let _ = fs::read(\"x\"); }",
        "use clap::Parser;",
        "use anyhow::Result;",
    ] {
        assert!(!violations(snippet).is_empty(), "missed: {snippet}");
    }
}

#[test]
fn the_checker_flags_a_wrapped_grouped_import() {
    let snippet = "use std::{\n    env,\n    fs,\n};\n";
    assert!(
        !violations(snippet).is_empty(),
        "missed a wrapped grouped import"
    );
}

#[test]
fn the_checker_allows_the_pure_forms() {
    for snippet in [
        "use std::collections::BTreeMap;",
        "use std::path::{Path, PathBuf};",
        "use std::io::Error;",
        "fn f(env: &str) -> String { env.to_owned() }",
        "// the process environment is the CLI's job",
        "crate::domain::taskenv::InstallPlan",
    ] {
        assert!(violations(snippet).is_empty(), "false positive: {snippet}");
    }
}
