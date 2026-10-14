# Maintainer review — map

Type: map
Status: complete

A Rust/open-source-maintainer review of momo at 0.5.0, taken on 2026-10-14, turned into a ticket batch. Three tiers: quick gate/hygiene wins, hardening with a decision or two, and longer-horizon structure. 18 tickets, delivered across 16 PRs, every one merged on green checks.

## Notes

- **Origin:** a read of the whole tree — source, tests, docs, CI, release workflow, ADRs and the hardening triage. `cargo fmt --check` and `cargo clippy -D warnings` were both clean on `main` at 0.5.0.
- **Landing:** jj only. Agents may push feature branches, open PRs and merge green PRs; never push to `main`.
- **No AI reviewer:** the merge gate is the required checks alone — recorded in `AGENTS.md`, which overrides the `ticket-batch` skill's review-drain steps for this repo.
- **Delivery:** one jj workspace per group, one agent per workspace, all based on `main` and grouped so each owned a disjoint file set. Merges serialised because required checks are strict up-to-date; each branch was rebased as siblings landed.

## Completed

| Ticket | PR | Outcome |
|--------|----|---------|
| 01 Enforce formatting in CI | #62 | `check` runs `fmt:check`; `fmt` stays the writer |
| 02 `missing_docs` lint | #62 | added; the public API was already documented |
| 03 rustdoc link check | #62 | `check` runs `cargo doc -D warnings` |
| 04 macOS test job | #66 | `check` is now a `[ubuntu, macos]` matrix |
| 05 CI concurrency + action pins | #66 | concurrency group; every action SHA-pinned |
| 06 Dependabot + cargo-deny | #66 | both added; Dependabot opened #68–#71 |
| 07 Release profile | #62 | `strip`/`lto`/`codegen-units`; binary −37% |
| 08 Release checksums | #66 | `SHA256SUMS` attached to the release |
| 09 Doc version drift | #62 | version-agnostic install snippets |
| 10 Changelog property tests | #67 | 7 invariants; no counterexamples at 200k cases |
| 11 Safe process-group kill | #61 | `rustix` `kill_process_group`; no shell-out |
| 12 JSON contract tests | #65 | every reporting verb round-tripped via `serde_json` |
| 13 Library API contract | #75 | full shrink to bin-shaped; two tests moved inline |
| 14 License | #73 | relicensed to `MIT OR Apache-2.0`; ADR 0004 |
| 15 Split large modules | #76 | changelog split (parse/render/footer); **3 modules remain** |
| 16 Non-Unix support | #72 | crate-root `compile_error!` on non-Unix |
| 17 Repo hygiene | #63 | research/prototypes untracked, kept on disk |
| 18 Dependabot auto-merge | #74 | `dependabot-automerge.yml`; merged on green |

Dependabot bumps #68 (`download-artifact`), #69 (`mise-action`), #70 (`upload-artifact`) and #71 (`checkout`) landed via auto-merge once #74's workflow existed.

## Decisions (2026-10-14)

| Ticket | Ruling |
|--------|--------|
| 13 | **Shrink to bin-shaped** — `domain`/`adapters` are `pub(crate)`; only `run()` is exported. The two integration tests that used the library API were moved inline. |
| 14 | **Relicense to `MIT OR Apache-2.0`** — sole human author confirmed. |
| 16 | **Declare Unix-only** — a non-Unix build fails with a clear compile error. |
| 17 | **Keep the issue files, untrack research/prototypes** — non-destructively. |

## Closed out (2026-10-14)

- **15 complete.** After `changelog.rs` (#76), the remaining modules were split: `cli/release.rs` → prepare/manifest/tag (#79), `domain/config.rs` → keys/env/paths (#78), and `cli/output.rs`'s tests → `output/tests.rs` (#81).
- **12 done.** The hand-rolled JSON serializer was replaced with `serde`/`serde_json` (#80); the `docs/ci.md` contract is byte-for-byte unchanged.
- **13 verified.** The shrink surfaced 8 dead-code items, all used by the test build, so `#[cfg_attr(not(test), allow(dead_code))]` on the two private module roots is the idiomatic fix — confirmed correct, no change needed.
