# 46: The `manifest` override example in the docs is wrong

Type: bug
Status: resolved
Severity: low
Area: docs
Verified: yes — read `docs/configuration.md:55` and `src/adapters/manifest/cargo.rs:29-34`
Found by: triage of ticket 24 (batch 4)

## Symptom

`docs/configuration.md` documents the per-package `manifest` override with a
repo-relative path, but the value is joined onto the *package directory*. A user
copying the example gets a doubled path that never resolves, and the override
silently does nothing (the manifest is then auto-discovered).

## Evidence

`docs/configuration.md:51-56`:

```toml
[packages."cargo:momo"]
identity = "cargo:renamed"            # override the identity
slug = "alpha"                        # override the ref/path-safe slug
manifest = "crates/momo/Cargo.toml"   # override the discovered manifest
```

`src/adapters/manifest/cargo.rs:29-34`:

```rust
fn locate(&self, package_dir: &Path, override_path: Option<&Path>) -> Option<PathBuf> {
    let path = match override_path {
        Some(override_path) => package_dir.join(override_path),
        None => package_dir.join(MANIFEST),
    };
```

For the package at `crates/momo`, the documented value resolves to
`crates/momo/crates/momo/Cargo.toml`.

## Trigger

Copy the documented example into `momo.toml` for a package rooted at
`crates/momo`. `momo release` does not sync that package's manifest.

## Suggested direction

Fix the example to be package-relative (e.g. `Cargo.toml`, or `../other/Cargo.toml`
if a sibling is genuinely intended), and state in the prose that the value is
resolved relative to the package directory. Ticket 24's path validation should
decide whether `..` is allowed here.

## Open questions for triage

- None — this is a documentation correction. It should land with ticket 24 so the
  path semantics are documented once.

## Resolution

`docs/configuration.md` now shows `manifest = "Cargo.toml"` and states that the
value is resolved relative to the package directory, spelling out the
repo-relative mistake (`crates/momo/Cargo.toml` → doubled path). The per-package
path is now also validated as relative (ticket 24).
