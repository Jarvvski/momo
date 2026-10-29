# momo bug hunt — 2026-10-27

Type: map
Status: resolved

A fresh bug hunt over momo 0.7.0, after the 2026-10-13 hardening effort closed (48 tickets, 45 resolved, 3 `wontfix`, shipped as 0.3.0). Four read-only investigators swept the manifest adapters, the workspace/graph layer, the release/config CLI, and the VCS/output/diagnostic layer. Every finding below was then re-read in the source by hand and, where the tooling allowed, reproduced — against the built binary, the installed jj 0.46.0, and the live GitHub anchor rendering. Nothing here is an investigator assertion taken on faith.

## What is here

Fourteen tickets under `issues/`, each carrying its `Status:`, `Severity:`, `Area:` and a `Verified:` line saying how the finding was confirmed.

## Status roll-up

All fourteen are fixed and landed, each on its own branch and PR, run by a solo sub-agent in its own jj workspace:

| Ticket | Fix | PR |
|--------|-----|----|
| 01 | Create the changelog's parent directory when recording | #118 |
| 02 | Report an inherited Cargo rename under its real package | #120 |
| 03 | Compare Python specifiers with PEP 440's ordering key | #119 |
| 04 | Fix the Poetry caret upper bound for `^0` and `^0.0` | #117 |
| 06 | Parse the jj remote URL without its push annotation | #121 |
| 08 | Point error doc links at the anchor GitHub generates | #122 |
| 09 | Do not leave a generated `momo.toml` behind when a change fails | #123 |
| 10 | Remove every directory release staging created on discard | #124 |
| 11 | Reject tag names `git check-ref-format` rejects | #125 |
| 12 | Write `momo.toml` atomically | #127 |
| 13 | Detect a mise config that lives only under `conf.d` | #126 |

| 05 | Namespace the Changelog and ledger when Packages share a directory | #134 |
| 07 | Seed every Package's ledger with `init --version` | #132 |
| 14 | Resolve the project root at the mise monorepo root | #133 |

The three that needed a human ruling — 05, 07 and 14 — were ruled on 2026-10-29 and fixed in a second wave. Nothing is open.

## Grouping into PRs

One focused change per branch, so each ticket lands on its own.

| Wave | Tickets | Theme |
|------|---------|-------|
| 1 | 01, 02, 03, 04 | The high-severity four: a core verb broken by config, a silent cascade false negative, and two Python range bugs |
| 2 | 06, 07, 08 | Medium: jj remote parse, `init --version` ledger, error doc anchors |
| 3 | 09, 10, 11, 12, 13 | Low: failed-add config, dry-run dirs, ref-name validation, atomic config, `conf.d` detection |
| 4 | 05, 07, 14 | Ruled 2026-10-29: shared-directory namespacing, per-package `init --version` seed, mise-root resolution |

## Fog / leads

- Not hunted this pass: `src/cli/output.rs` serialisation beyond field-shape spot-checks, the Python `pyproject.toml` writer, and the Go module-path edge cases. A second sweep could cover them.

## Method

Nine targeted investigators in the 2026-10-13 effort; four this time, one per layer. Findings were ranked by the investigator, then each was re-read at its cited lines and reproduced where cheap. The Python PEP 440 and Poetry-caret claims were checked against `packaging` and `poetry-core` semantics; the jj claim against jj 0.46.0; the anchor claim against the live rendered page; the `momo add` claims against the built binary.
