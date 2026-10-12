# Rust crate & dependency landscape

Type: research
Status: resolved

## Question

Recommend momo's v0 dependency set, with trade-offs:

- **CLI parsing:** `clap` vs `argh` vs `lexopt`.
- **TUI multi-select:** `inquire` vs `dialoguer` vs `ratatui`.
- **TOML round-trip editing:** `toml_edit` (comment/format preserving).
- **Semver:** the `semver` crate + bump logic.
- **Dates/timestamps:** `time` vs `chrono`.
- **Git access:** `git2` vs shelling out to `git`.
- **jj access:** CLI.
- **Errors:** `anyhow` at the top level, `thiserror` for typed errors (per
  `AGENTS.md`).

Flag anything that fights the repo's constraints: `unsafe_code = "forbid"`,
edition 2024, Rust 1.99, clippy warnings-as-errors.

## Answer

Full findings: [`../research/rust-crates.md`](../research/rust-crates.md).

Recommended v0 dependency set:

- **CLI:** `clap` with `features = ["derive"]`.
- **Task-env picker:** `inquire`, trimmed
  (`default-features = false, features = ["crossterm"]`) — drop it entirely if
  ticket 08 lands on detect/flags-only.
- **TOML:** `toml_edit` (`DocumentMut`); mutate in place — never round-trip
  through `serde`.
- **Semver:** `semver` 1.0 — a parser/evaluator with **no bump API**; momo owns
  the bump policy as a pure, unit-tested function.
- **Dates:** `time` 0.3, no features (UTC `Date` Displays as `YYYY-MM-DD`).
- **VCS:** shell out to **both** `git` and `jj` through one `Command` seam — no
  `git2` (avoids a C build + FFI, and jj has no Rust lib).
- **Errors:** `anyhow` top-level + `thiserror` for typed domain errors.

`unsafe_code = "forbid"` is a **per-crate** lint, so it does not constrain
dependencies (it is a signal, not a blocker).

Cross-reference: confirmed `jj tag set | list | delete` exists (jj 0.46), with
`--allow-move` for the "tag exists" path — feeds tickets 12 and 06.
