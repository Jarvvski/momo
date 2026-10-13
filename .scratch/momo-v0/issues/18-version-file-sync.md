# Version-file detection & sync

Type: grilling
Status: resolved
Blocked by: 16, 20

## Question

Stretch goal (user-flagged): momo should **detect where the version is defined**
(`Cargo.toml`, `gradle.properties`, `package.json`, …) and **set it** to the
minted version, so the build picks it up without extra wiring.

Settle:

- **Scope:** is this v0, or post-v0? (The user called it a "big stretch goal".)
- Which **ecosystems / manifests** in v0?
- **Detection:** auto-discover the manifest(s), or config-declared?
- **Writing:** format-preserving where possible (`toml_edit` for TOML; what about
  JSON / `.properties`?), and how to avoid clobbering a hand-formatted file.
- Relationship to ticket 06 (release side-effects) — sync is a release step.
- How this composes with the monorepo model (ticket 16) — one manifest per package.

## Answer

Settled (all recommendations accepted):

1. **Scope: minimal v0 (option c).** Ship the mechanism — a `ManifestSync` trait
   mirroring `TaskEnvironment` — plus **Rust `Cargo.toml`**. Node/Gradle/Python are
   later impls with no core change.
2. **Detection: auto-discover per package** (look for the known manifest in each
   package root), config-overridable, skipping gracefully when none is found.
3. **Writing: format-preserving** (ticket 20) — `toml_edit` for `Cargo.toml`, never
   reserialize, touch only the version scalar. Lockfiles (`Cargo.lock`) are left to
   the package manager: momo **warns** they may be stale rather than running
   `cargo`. Sync is a **release step** (ticket 06); `status`/`--dry-run` never write.
