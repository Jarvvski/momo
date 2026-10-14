# momo v1 — roadmap effort

Type: map
Status: active

The v1 roadmap: features, correctness fixes and posture changes synthesised from
five investigator reports — a competitive teardown, the mise/jdx ecosystem, the
opinionation playbook, polyglot-monorepo release pain, and a differentiator gap
analysis. Seventeen tickets live in `issues/`; one is `wontfix`.

## Pointing and order

Every ticket carries a `Points:` line — the value-weighted score from the
evaluation:

    Points = Value×2 + Ease + Novelty        (each axis 1–5; Ease 5 = easiest)

Higher is better. Tickets are **numbered in points order** (`01` = highest), so
this repo's frontier rule — *the lowest-numbered open, unblocked, unclaimed
ticket* — already walks the work best-first. Ties break by value, then ease, then
novelty.

`Blocked by:` gates a ticket on another: a blocked ticket is skipped until every
file it lists is `resolved`. Three **low-pointed prerequisites** therefore run
before the high-value tickets they unlock — ticket 14 (generalise the
`ManifestSync` port) gates 04 and 08; ticket 03 (workspace-graph fields) gates 07
and 12; the `INSTALLABLE_VERBS` split folded into ticket 10 gates 16. That is
correct, not a contradiction: **points rank value, blockers rank order.**

To re-point the effort, edit the `Points:` lines and re-sort; the numbers are the
only priority mechanism, and the ranking in this map is derived from them.

## Scoring matrix

| # | Ticket | Pts | N | E | V | Status | Blocked by |
|---|--------|:---:|:-:|:-:|:-:|--------|-----------|
| 01 | [Option philosophy + freeze the config surface](issues/01-option-philosophy-freeze.md) | 17 | 4 | 5 | 4 | ready-for-human | — |
| 02 | [Refuse to mint from unread changelog sections](issues/02-refuse-unread-changelog-sections.md) | 16 | 3 | 3 | 5 | ready-for-agent | — |
| 03 | [Consume the workspace-graph fields](issues/03-workspace-graph-fields.md) | 16 | 4 | 4 | 4 | ready-for-agent | — |
| 04 | [Node `package.json` manifest adapter](issues/04-node-manifest-adapter.md) | 14 | 1 | 3 | 5 | ready-for-agent | 14 |
| 05 | [Publish GitHub artifact attestations](issues/05-github-attestations.md) | 14 | 2 | 4 | 4 | ready-for-agent | — |
| 06 | [Warn on an unresolvable Go tag prefix](issues/06-go-tag-prefix-warn.md) | 14 | 3 | 5 | 3 | ready-for-agent | — |
| 07 | [Cascade: rewrite dependent ranges](issues/07-cascade-rewrite-ranges.md) | 13 | 1 | 2 | 5 | ready-for-agent | 03, 14 |
| 08 | [Python `pyproject.toml` manifest adapter](issues/08-python-manifest-adapter.md) | 13 | 1 | 4 | 4 | ready-for-agent | 14 |
| 09 | [Make `status` the blessed path; pin zero-config](issues/09-status-blessed-path.md) | 13 | 2 | 5 | 3 | ready-for-agent | — |
| 10 | [jdx CLI citizenship: `version`, help examples, `--json`](issues/10-jdx-cli-citizenship.md) | 13 | 2 | 5 | 3 | ready-for-agent | — |
| 11 | [State the release opinions](issues/11-state-release-opinions.md) | 13 | 3 | 4 | 3 | ready-for-human | — |
| 12 | [Cascade: report unsatisfied dependent ranges](issues/12-cascade-report-ranges.md) | 12 | 1 | 3 | 4 | ready-for-agent | 03 |
| 13 | [Teaching errors](issues/13-teaching-errors.md) | 12 | 2 | 2 | 4 | ready-for-agent | — |
| 14 | [Generalise `ManifestSync` to a plan](issues/14-manifest-port-plan.md) | 11 | 1 | 4 | 3 | ready-for-agent | — |
| 15 | [Derive the Go tag prefix; per-package override](issues/15-go-tag-prefix-derive.md) | 11 | 2 | 3 | 3 | ready-for-human | — |
| 16 | [Shell completions](issues/16-shell-completions.md) | 10 | 2 | 4 | 2 | ready-for-agent | 10 |
| 17 | [Promote `.momo/version` to a version authority](issues/17-version-authority-side-manifest.md) | — | 1 | — | 1 | wontfix | — |

## Buckets

Tickets group into five buckets by kind of work. Buckets drive PR grouping; points
drive order.

| Bucket | Tickets |
|---|---|
| Monorepo release engine | 03, 07, 12 |
| Ecosystem manifest adapters | 04, 06, 08, 14, 15, 17 |
| Opinionated posture | 01, 09, 11 |
| Trust & correctness | 02, 13 |
| jdx ecosystem fit | 05, 10, 16 |

## Frontier and gating

Initial frontier — open, unblocked, in points order: **01, 02, 03, 05, 06, 09, 10,
11, 13, 14, 15**. Gated behind a prerequisite: 04 (→14), 07 (→03,14), 08 (→14),
12 (→03), 16 (→10).

Suggested landing groups, coherent and reviewable:

| Group | Tickets | Theme |
|-------|---------|-------|
| A | 01, 09, 11 | Posture: philosophy, blessed path, stated opinions (docs + tests) |
| B | 03, 12, 07 | Monorepo engine: graph fields → report → rewrite |
| C | 14, 04, 08, 15 | Adapters: port plan → Node → Python → Go derive |
| D | 13, 02 | Trust: error codes, then the changelog refusal that rides them |
| E | 06 | Go tag-prefix warning (independent, free-standing) |
| F | 05, 10, 16 | jdx fit: attestations, CLI citizenship, completions |

## Rejected and deferred

- **17 — promote `.momo/version` to a version authority: `wontfix`.** The premise
  is a misconception (momo's adapters are write-only); the real need is
  skip-visibility, delivered by ticket 14. See the ticket.
- **Cascade force-release** (manufacture a dependent bump with no recorded
  Change): contradicts the documented v0 stance (`docs/usage.md:87`,
  `spec.md:134`, issue 16). An owner ruling, not scheduled.
- **`--csv`, report colour, tables, prompts:** rejected — conflicts with
  short-but-sweet and the report-only stance.
- **Deferred post-v1:** a docs site, adopting `usage` (KDL), a first-class GitHub
  Action, a `setup.cfg` (INI) locator.

## Notes

- The evidence behind each ticket is one of the five investigator reports
  (competitive teardown, mise/jdx ecosystem, opinionation playbook, polyglot
  release pain, differentiator gap analysis). Each ticket cites the file paths
  the investigator read; re-verify against the source before implementing.
- Scoring is judgement, not measurement. The `## Scoring` section in each ticket
  records the axis values so the score can be re-derived or re-pointed.
- `docs/agents/triage-labels.md` maps the canonical roles onto the `Status:`
  strings used here.
