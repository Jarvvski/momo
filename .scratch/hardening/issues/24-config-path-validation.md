# 24: `changelog` and `changes_dir` accept absolute and `..` paths

Type: bug
Status: resolved
Severity: medium
Area: domain/config + domain/package
Verified: yes — read at the cited lines

## Symptom

The path keys are taken verbatim and joined onto the package or repo root. An
absolute path discards the base entirely, and `..` escapes upward — so a config
value can silently write outside the package, contradicting the documented
"relative to the package root / repo root".

## Evidence

`src/domain/config.rs:417-419`:

```rust
fn parse_path(doc: &DocumentMut, key: &str) -> Result<Option<PathBuf>, ConfigError> {
    Ok(string_at(doc, key)?.map(PathBuf::from))
}
```

`src/domain/package.rs:111` `self.dir(repo_root).join(&config.changelog)` and
`:118` `repo_root.join(&config.changes_dir)`. `Path::join` with an absolute
argument returns the argument.

## Trigger

`changelog = "/tmp/x.md"` → the release writes `/tmp/x.md`.
`changes_dir = "../shared-changes"` → fragments land outside the repo.
`MOMO_CHANGELOG=/tmp/x.md` does the same (see ticket 19).

## Suggested direction

Reject absolute and parent-escaping paths at config parse time with a clear error,
or document explicitly that they are honoured.

## Open questions for triage

- Reject, or allow-and-document? A monorepo might legitimately want a shared
  changelog directory, which argues for allowing `..` under a documented rule.

## Triage

**Verdict:** Confirmed. `parse_path` (`config.rs:417-419`) takes the value
verbatim; `Package::changelog` and `changes_dir` (`package.rs:110-124`) `join` it
onto the package/repo root, and `Path::join` with an absolute argument discards
the base while `..` escapes upward. So `changelog = "/tmp/x.md"` writes
`/tmp/x.md` and `changes_dir = "../shared-changes"` lands fragments outside the
repo.

**Corrections:** The ticket cites only the file keys; the same validation must
cover the env keys (`MOMO_CHANGELOG`/`MOMO_CHANGES_DIR`, see 19) and the
per-package `manifest` override, which is also joined to `package_dir`
(`cargo.rs:31`). The "shared changelog directory" argument for allowing `..` is
weak: ticket 16 gives each package its own changelog, and `changes_dir` is
already repo-root-relative with a per-slug namespace
(`package.rs:117-123`), so a monorepo does not need `..` to share a fragment
store.

**Ruling: reject** absolute and `..`-escaping paths at parse time with a clear
error, for `changelog`, `changes_dir` (file and env) and the per-package
`manifest`. Do not allow-and-document — silent writes outside the tree are
precisely the surprise to avoid.

**Related doc bug:** `docs/configuration.md:55` shows
`manifest = "crates/momo/Cargo.toml"` under `[packages."cargo:momo"]`, but
`manifest` is joined to the *package* directory, so for a package rooted at
`crates/momo` that resolves to `crates/momo/crates/momo/Cargo.toml`. The code
and `PackageConfig.manifest`'s doc comment agree on package-relative, so the doc
example is wrong and should be corrected (e.g. to `manifest = "Cargo.toml"` or a
package-relative name). Worth fixing in the same PR.

**Scope:** Overlaps 19 (same `validate_path` helper); keep separate. Not a
duplicate of 15 — 15 is a *derived* workspace-root write, 24 is *user-configured*
paths.

**Implementation sketch:**

- Add `ConfigError::InvalidPath { key: String, value: String }` with
  `#[error("`{key}` must be a relative path without `..`: `{value}`")]`.
- Add `fn validate_path(key: &str, path: PathBuf) -> Result<PathBuf, ConfigError>`
  rejecting `path.is_absolute()` and any `std::path::Component::ParentDir`.
- Call it from `parse_path`; from the `MOMO_CHANGELOG`/`MOMO_CHANGES_DIR` arms of
  `from_env`; and from `parse_packages` for `manifest` (key
  `packages.<id>.manifest`). Share the helper with 19's empty-value rejection.
- Tests: absolute `changelog` errors; `changes_dir = "../x"` errors;
  `MOMO_CHANGELOG=/tmp/x` errors; `packages."cargo:momo".manifest = "a/../b"`
  errors; an ordinary relative path still parses.

## Resolution

Added `validate_path` (shared with 19), rejecting empty paths
(`ConfigError::EmptyPath`) and absolute / `..`-escaping paths
(`ConfigError::InvalidPath`). It is called from `parse_path` (file `changelog` and
`changes_dir`), from the `MOMO_CHANGELOG`/`MOMO_CHANGES_DIR` env arms, and from
`parse_packages` for the per-package `manifest`. Tests cover each key plus the
happy path. `docs/configuration.md` now states the rule and the package-relative
`manifest` semantics (see 46).
