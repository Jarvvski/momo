# Version-file detection & sync

Type: grilling
Status: claimed
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
