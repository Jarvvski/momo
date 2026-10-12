# momo v0 — spec

> **Status:** in progress. Assembled as the decision tickets in
> `.scratch/momo-v0/issues/` resolve; each open section points at its ticket.
> The map is `.scratch/momo-v0/map.md`. Vocabulary is in `GLOSSARY.md`.

## What momo is

A short, standalone Rust CLI that turns a project's accumulated changes into
semver releases, a changelog, and VCS tags. It runs standalone (`cargo install`)
or integrates into a task environment (`mise` first).

## Scope (v0)

- Single binary, Rust (edition 2024, `unsafe_code = "forbid"`).
- Task-environment integration via an internal `TaskEnvironment` trait, with
  `mise` as the only implementation. No third-party plugin loading.
- Standalone use needs no task environment.

## Command surface

| Command | Purpose |
|---|---|
| `momo break\|add\|fix "<desc>"` | Record a change |
| `momo status` | Show unreleased changes + the version they'd mint |
| `momo release [--rc]` | Mint a version, timestamp the changelog, optionally tag |
| `momo rc` | Cut / increment a release candidate |
| `momo tag` | Create the VCS tag for the current version |
| `momo init` | Scaffold changelog + config |
| `momo install <taskenv>` | Install integration into a task environment |
| `--dry-run` | Global: show what would happen, change nothing |

## Semver rules

- **MAJOR 0:** `break` and `add` both bump MINOR; `fix` bumps PATCH.
- **MAJOR ≥ 1:** `break` bumps MAJOR; `add` bumps MINOR; `fix` bumps PATCH.
- Edge cases (`0.0.x`, multiple kinds per release) → see tickets 01 / 05.

## Open design areas

Each is decided by a ticket; the spec gains its section as the ticket resolves.

| Area | Ticket |
|---|---|
| Version source of truth & first-release bootstrap | 01 |
| Recording model (single changelog vs fragments) | 02 |
| Changelog format & entry shape | 03 |
| Config file & keys | 04 |
| Release-candidate semantics | 05 |
| Release side-effects & tag format | 06 |
| CI / non-interactive output & dry-run | 07 |
| Task-environment selection UX | 08 |
| `mise` integration content | 09 |
| Repo & module shape | 10 |
| CLI ergonomics | 11 |
| jj & git tag mechanics *(research)* | 12 |
| Precedents: existing versioning tools *(research)* | 13 |
| Rust crate & dependency landscape *(research)* | 14 |

## Assembly

Ticket 15 consolidates the resolved decisions into this document.
