# 02: Enforce the missing_docs convention

Type: task
Status: ready-for-agent
Severity: medium
Area: tooling
Verified: yes — `[lints.rust]` in `Cargo.toml` sets only `unsafe_code` and `unused`; `mise run lint` runs `cargo clippy --all-targets -- -D warnings`.

## Symptom

`docs/contributing.md` and `AGENTS.md` both require public items to be doc-commented, but nothing enforces it. A new public item can land undocumented.

## Suggested direction

Add `missing_docs = "warn"` to `[lints.rust]`. `cargo clippy -- -D warnings` forwards `-D warnings` to rustc, so the warning becomes an error in `mise run lint` and CI. The public API (`domain`, `adapters`) already looks documented; private modules (e.g. `cli`) are not reachable from the crate root and are exempt. Fix any item the lint surfaces rather than adding `#[allow]`.

## Related

- 13 (the library's public-API contract), Group A.
