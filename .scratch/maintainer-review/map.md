# Maintainer review — map

Type: map
Status: open

A Rust/open-source-maintainer review of momo at 0.5.0, taken on 2026-10-14, turned into a ticket batch. Three tiers: quick gate/hygiene wins, hardening with a decision or two, and longer-horizon structure. 17 tickets, grouped into five agent workspaces plus a decision queue and one deferred refactor.

## Notes

- **Origin:** a read of the whole tree — source, tests, docs, CI, release workflow, ADRs and the hardening triage. `cargo fmt --check` and `cargo clippy -D warnings` are both clean on `main`.
- **Standing preference:** small, reviewable PRs; the repo's own conventions (`AGENTS.md`, `docs/contributing.md`) win over anything here.
- **Landing:** jj only. Agents may push feature branches, open PRs and merge green PRs; never push to `main`. See `AGENTS.md`.

## Groups

Grouped so that each workspace owns a disjoint set of files — the conflicts that remain are between independent axes, not artefacts of splitting one change in two.

| Group | Workspace | Tickets | Owns |
|-------|-----------|---------|------|
| A Gates | `gates` | 01, 02, 03, 07, 09 | `mise.toml`, `Cargo.toml` (lints + profile), `README.md`, `docs/ci.md` |
| B CI & supply chain | `ci-supply` | 04, 05, 06, 08 | `.github/**`, `deny.toml` |
| C Changelog properties | `changelog-props` | 10 | `tests/changelog_properties.rs`, `Cargo.toml` (dev-deps) |
| D Process-group kill | `spawn-kill` | 11 | `src/adapters/spawn.rs`, `Cargo.toml` (deps) |
| E JSON contract | `json-contract` | 12 | `src/cli/output.rs`, `tests/` |

`Cargo.toml` is touched by A, C and D, but in different sections (`[lints]`/`[profile]`, `[dev-dependencies]`, `[dependencies]`); reconcile on rebase if the hunks meet.

## Decisions (2026-10-14)

Cleared in one batched round. Each ruling is recorded in its ticket.

| Ticket | Ruling | Now |
|--------|--------|-----|
| 13 | **Shrink to bin-shaped** — `domain`/`adapters` become `pub(crate)`; expose only `run()` | ready-for-agent, sequenced after 11 + 12 |
| 14 | **Relicense to `MIT OR Apache-2.0`** (confirm sole-author history first) | ready-for-agent, sequenced after 09 + 06 |
| 16 | **Declare Unix-only** — `compile_error!` on non-Unix | ready-for-agent, sequenced after 11 + 09 |
| 17 | **Keep the issue files, untrack research/prototypes** (non-destructively) | ready-for-agent, in flight |

13, 14 and 16 are held: each touches files an in-flight workspace owns, so landing them now would be a semantic conflict, not a merge artefact. 17 is independent and runs now.

## Deferred

- **15 — split the >1000-line modules.** Agent-ready, but must wait for 10 and 12 to land: it touches `changelog.rs` and `output.rs`, which they change.

## Decisions so far

- **2026-10-14** — all four rulings recorded above. Tickets 13, 14 and 16 reclassified `ready-for-human` → `ready-for-agent` with their sequencing constraint written in; 17 spawned immediately.
