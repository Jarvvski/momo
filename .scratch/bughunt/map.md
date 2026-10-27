# momo bug hunt — 2026-10-27

Type: map
Status: open

A fresh bug hunt over momo 0.7.0, after the 2026-10-13 hardening effort closed (48 tickets, 45 resolved, 3 `wontfix`, shipped as 0.3.0). Four read-only investigators swept the manifest adapters, the workspace/graph layer, the release/config CLI, and the VCS/output/diagnostic layer. Every finding below was then re-read in the source by hand and, where the tooling allowed, reproduced — against the built binary, the installed jj 0.46.0, and the live GitHub anchor rendering. Nothing here is an investigator assertion taken on faith.

## What is here

Fourteen tickets under `issues/`, each carrying its `Status:`, `Severity:`, `Area:` and a `Verified:` line saying how the finding was confirmed.

## Status roll-up

- **ready-for-agent:** 01, 02, 03, 04, 06, 07, 08, 09, 10, 11, 12, 13 (12)
- **ready-for-human:** 05 (needs a ruling: refuse vs namespace)
- **needs-triage:** 14 (reported, not reproduced)

## Grouping into PRs

One focused change per branch, so each ticket lands on its own.

| Wave | Tickets | Theme |
|------|---------|-------|
| 1 | 01, 02, 03, 04 | The high-severity four: a core verb broken by config, a silent cascade false negative, and two Python range bugs |
| 2 | 06, 07, 08 | Medium: jj remote parse, `init --version` ledger, error doc anchors |
| 3 | 09, 10, 11, 12, 13 | Low: failed-add config, dry-run dirs, ref-name validation, atomic config, `conf.d` detection |
| — | 05 | Blocked on the refuse-vs-namespace ruling |

## Fog / leads

- Ticket 14 is the one finding not reproduced by hand; it needs a repro before it is actionable.
- Not hunted this pass: `src/cli/output.rs` serialisation beyond field-shape spot-checks, the Python `pyproject.toml` writer, and the Go module-path edge cases. A second sweep could cover them.

## Method

Nine targeted investigators in the 2026-10-13 effort; four this time, one per layer. Findings were ranked by the investigator, then each was re-read at its cited lines and reproduced where cheap. The Python PEP 440 and Poetry-caret claims were checked against `packaging` and `poetry-core` semantics; the jj claim against jj 0.46.0; the anchor claim against the live rendered page; the `momo add` claims against the built binary.
