# 08: Manifest version sync (Cargo)

**What to build:** A `ManifestSync` port in `momo-core` and a Rust
implementation. `release` auto-discovers the Package's `Cargo.toml` and updates
only the version scalar, format-preserving via `toml_edit` — never
reserialising. The manifest is overridable in config and skipped gracefully when
none is found. Lockfiles are left to the package manager: momo warns that
`Cargo.lock` may be stale rather than running `cargo`. Sync is a release step
only; `status` and `--dry-run` never write.

**Blocked by:** 04

**Spec:** `.scratch/momo-v0/spec.md` §11

**Status:** resolved

- [x] `momo release` updates the version in `Cargo.toml` while preserving comments, ordering and whitespace.
- [x] No other key in the manifest is touched.
- [x] A stale `Cargo.lock` produces a warning, not a rewrite.
- [x] With no manifest present, release succeeds and skips sync.
- [x] `momo status` and `--dry-run` never write a manifest.

## Comments

Implemented as a new `momo-manifest` adapter crate (mirroring `momo-vcs` /
`momo-taskenv`) behind a `ManifestSync` port in `momo-core`. The write resolves
the authoritative version key: `[package].version` when it is a plain string,
else `[workspace.package].version` for a member that inherits
(`version.workspace = true`) or a virtual manifest — the workspace pitfall in
`research/version-file-formats.md`. A manifest with neither key is skipped, never
grown a new key. The path is overridable per Package via
`[packages."<identity>"] manifest = "..."` in `momo.toml`.
