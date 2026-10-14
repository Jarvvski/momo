# momo hardening — triage effort

Type: map
Status: complete

A bug hunt over the whole codebase on 2026-10-13 produced 45 tickets, triaged in
six batches by fresh sub-agents, plus 3 tickets the triage itself surfaced (46,
47, 48). 48 tickets total: **45 resolved, 3 `wontfix`**, shipped across 16 PRs.

## What landed

### First wave

Eight groups shipped as PRs #40–#47, each implemented in its own jj workspace and
merged once `check` was green.

| Group | PR | Tickets |
|-------|----|---------|
| 1 Release integrity | #43 | 37, 35, 47 |
| 2 Version guards | #44 | 05, 08 |
| 3 Changelog fidelity | #42 | 09, 11, 12, 14 |
| 4 Package identity | #40 | 16, 17, 18 |
| 5 Config integrity | #47 | 19, 20, 21, 22, 23, 24, 46 |
| 6 VCS hardening | #41 | 26, 27, 28, 31 |
| 7 Process & mise | #46 | 30, 32, 33, 34 |
| 8 Test & tooling | #45 | 40, 41, 43, 44, 45 |

Released as **0.3.0**.

### Second wave

The eleven `ready-for-human` tickets were ruled on 2026-10-14 (see the decisions
table below) and shipped as PRs #50–#55.

| Group | PR | Tickets |
|-------|----|---------|
| 9 Multi-package output | #50 | 03, 42 |
| 10 jj tag anchor | #51 | 25 |
| 11 Shared workspace version | #52 | 15 |
| 12 Changelog document model | #53 | 38, 10 |
| 13 Version model | #54 | 36, 04, 06, 07 |
| 14 Process seam | #55 | 39, 48 |

Several of these merged under each other and needed their conflicts reconciled by
hand — the changelog redesign against the ledger accessor, the `create_tag`
signature against the ledger, and the output aggregation against both the
`prepare`/`commit` split and the ledger-aware `tag`. Each reconciliation kept both
intents; none deleted an assertion.

One defect reached CI and was fixed there: the new command timeout killed the
direct child but not a forked grandchild, so `mise` (which forks) could still hold
the call open. The child now leads its own process group and the group is killed.

The CRLF question resolved as **(c)**: ticket 34 is fixed — the `Cargo.toml` edit
preserves line endings — and ticket 13's changelog case was then overtaken by the
changelog redesign, which preserves CRLF too.

## Method

Nine targeted investigators, one per layer. The top findings were then re-read in
the source by hand; each ticket carries a `Verified:` line saying whether the
reporter or a human confirmed it against the code. Six triage batches then
re-read every ticket against the cited code, corrected the bodies where the
report was wrong, and set a canonical `Status:`.

Triage found three reported "suggested directions" that were themselves wrong
(tickets 04, 08, 35) — reported fixes need the same verification as reported
bugs.

## Status roll-up

As triaged, before the owner's rulings and the second wave:

- **ready-for-agent:** 05, 08, 09, 11, 12, 14, 16, 17, 18, 19, 20, 21, 22, 23,
  24, 26, 27, 28, 30, 31, 32, 33, 34, 35, 37, 40, 41, 42, 43, 44, 45, 46, 47 (33)
- **ready-for-human:** 03, 04, 06, 07, 10, 15, 25, 36, 38, 39, 48 (11)
- **wontfix:** 01, 02, 13, 29 (4)

Final: **45 resolved, 3 `wontfix`**. Ticket 13 moved to resolved when 38 landed —
the redesign preserves CRLF after all (see the CRLF note below).

`wontfix` here does **not** mean "not a bug":

- **01, 02** — real, but superseded by 37. The staged transaction is the fix for
  both; implementing either separately would redo the same code.
- **13** — real; first ruled out of scope, then fixed by 38 (see the CRLF note
  below).
- **29** — jj's first-remote fallback is deliberate and pinned by a test.

## Owner decisions (2026-10-14)

All eleven were ruled; each ticket now carries a `## Decision` section.

| # | Ruling |
|---|--------|
| 03 | A `status`-style `packages` envelope — one shape across every reporting verb. Breaking change to `docs/ci.md`. |
| 04 | Recompute the target from the last release; if a cut RC disagrees, fail and ask for a new RC. |
| 36 | Pending-release ledger: `.momo/version` records the last *minted* release; `status`/`release` read tags; `tag` reads the ledger. |
| 06, 07 | Subsumed by 36. |
| 38 | Preserve the document: raw lines plus momo-owned spans, everything else byte-for-byte; fail loudly when an edit would touch foreign content. |
| 10 | Subsumed by 38. |
| 15 | Support the shared workspace version — resolve the workspace root and write `[workspace.package].version`, failing rather than letting `--all`'s last writer win. |
| 25 | Default the jj tag anchor to `@-`, with `--rev` for an uncommitted release; drop the dirtiness heuristic. |
| 39 | Reshape the ports: an opaque "operation failed" variant in the domain, process detail formatted at the CLI boundary. |
| 48 | Add a timeout (60s default, configurable), paired with 39's new error vocabulary. |

## Cross-batch conflict — CRLF (13 vs 34)

Batch 3 ruled ticket 13 (changelog CRLF) **wontfix + documented limitation**;
batch 5 ruled ticket 34 (`Cargo.toml` CRLF) **in scope**, and said the two "must
move together or the codebase is inconsistent". They cannot both be right.

The asymmetry that argues for treating them differently: `cargo.rs` *promises*
byte-for-byte preservation in its module doc (and `domain/manifest.rs` repeats
it), so 34 breaks a stated contract; the changelog makes no such promise. Options:

- **(a)** both `wontfix` + document the limitation;
- **(b)** both fixed (preserve line endings in each writer);
- **(c)** fix 34 (it contradicts a stated guarantee) and document 13.

Recommendation: **(c)**. Needs the owner's call.

**Outcome:** **(c)** — 34 was fixed (its writer preserves line endings) and 13 was
closed `wontfix` with a documented limitation. Then 38 landed the preserve model
and made that limitation moot: `Changelog` reapplies the document's own ending to
every line it writes, momo's own included, so 13 is `resolved` too and the two
writers are consistent. No "Not yet supported" note was needed, and none is
present.

## Grouping into PRs

Agent-ready groups, coherent and reviewable. Order is a suggestion — the release
integrity work is the natural first landing.

| PR | Tickets | Theme |
|----|---------|-------|
| 1 | 37, 35, 47 | Release integrity: staged transaction, atomic writes, inherited-version signal (closes 01, 02) |
| 2 | 05, 08 | Version guards: `rc_format` validation, overflow-safe bump |
| 3 | 09, 11, 12, 14 | Changelog fidelity: preserve the footer, match headings, sanitise text, fragments-mode guard |
| 4 | 16, 17, 18 | Package identity: slug collisions, identity/slug coupling, ambiguous `-p` |
| 5 | 19, 20, 21, 22, 23, 24, 46 | Config integrity: empty env, env-baking, dead keys, unknown keys, path validation, docs |
| 6 | 26, 27, 28, 31 | VCS hardening: remote exit status, SSH port, tag ordering, ref validation |
| 7 | 30, 32, 33, 34 | Process & mise: non-interactive env, detection names, graph dedup, manifest newlines |
| 8 | 40, 41, 43, 44, 45 | Test & tooling hardening: purity guard, skip gate, env coverage, assertions, date hoist |
| 9 | 03, 42 | Multi-package output (blocked on the shape ruling; 42 is its test) |

### Second wave (ruled 2026-10-14)

| Group | Tickets | Theme |
|-------|---------|-------|
| 10 | 36, 04, 06, 07 | Version model: the pending-release ledger and RC target recomputation |
| 11 | 38, 10 | Changelog document model: preserve raw lines and edit owned spans |
| 12 | 03, 42 | Multi-package output: the `packages` envelope, and its test |
| 13 | 39, 48 | Process seam: reshaped port errors and a command timeout |
| 14 | 25 | jj tag anchor: default `@-`, add `--rev` |
| 15 | 15 | Shared workspace version: resolve the root and write it |

## Notes

- **Sequencing constraints:** 42 must follow 03 (same code path). 47 and 37 both
  touch `sync_manifest`; 04 and 08 both reshape `release_version`. Land within a
  group, not across parallel branches.
- **Cheapest wins are doc-vs-code mismatches:** 28 (doc says "semver ordering",
  code uses derived `Ord`), 34 (doc says "byte-for-byte"), 30 (comment implies
  safety it lacks), 46 (wrong example).
- `docs/agents/triage-labels.md` maps the canonical roles onto this repo's
  `Status:` strings.
- After the agent-ready PRs land, cut a release with momo itself.
