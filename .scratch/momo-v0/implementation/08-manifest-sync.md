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

**Status:** ready-for-agent

- [ ] `momo release` updates the version in `Cargo.toml` while preserving comments, ordering and whitespace.
- [ ] No other key in the manifest is touched.
- [ ] A stale `Cargo.lock` produces a warning, not a rewrite.
- [ ] With no manifest present, release succeeds and skips sync.
- [ ] `momo status` and `--dry-run` never write a manifest.
