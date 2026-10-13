# momo hardening — triage effort

Type: map
Status: triaged

A bug hunt over the whole codebase on 2026-10-13 produced 45 tickets, triaged in
six batches by fresh sub-agents, plus 3 tickets the triage itself surfaced (46,
47, 48). 48 tickets total.

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

- **ready-for-agent:** 05, 08, 09, 11, 12, 14, 16, 17, 18, 19, 20, 21, 22, 23,
  24, 26, 27, 28, 30, 31, 32, 33, 34, 35, 37, 40, 41, 42, 43, 44, 45, 46, 47 (33)
- **ready-for-human:** 03, 04, 06, 07, 10, 15, 25, 36, 38, 39, 48 (11)
- **wontfix:** 01, 02, 13, 29 (4)

`wontfix` here does **not** mean "not a bug":

- **01, 02** — real, but superseded by 37. The staged transaction is the fix for
  both; implementing either separately would redo the same code.
- **13** — real, but ruled out of scope (CRLF, see the conflict note below).
- **29** — jj's first-remote fallback is deliberate and pinned by a test.

## Human decision queue

These need an owner ruling before they can become agent work. Each is one
question.

| # | Question |
|---|----------|
| 03 | What is the aggregate JSON shape for multi-package `release`/`candidate`/`tag`? (`status`-style `packages` envelope vs always-wrapped `releases`.) Breaking change to `docs/ci.md` either way. |
| 04 | When a `break` lands *after* an RC is cut, does the target move? Note the reporter's trigger was wrong: at MAJOR 0 it never moves; it only bites at MAJOR ≥ 1. |
| 36 | Which model — `.momo/version` derived from the tag, or fallback-only? Parent of 06 and 07. The spec ("VCS authoritative") is incoherent for `tag`, which needs a version no tag records. |
| 06, 07 | Subsumed by 36; keep as symptoms. |
| 38 | Changelog document model: preserve the raw document and edit owned spans, or fail loudly? Parent of 10 (and overlaps 09/11/14). Vocabulary ruled: three Kinds only. |
| 15 | Policy for a *shared* `[workspace.package].version` under per-package versioning: refuse, treat the workspace as one package, or allow a root write with an explicit conflict rule. |
| 25 | jj tag anchor: what is the correct rule — narrow the dirtiness heuristic, or make the revision explicit? Reshapes the `Vcs` port. |
| 39 | Reshape the domain port error vocabulary. Needs a ruling on how much process detail the CLI must show. |
| 48 | Timeout policy: default duration, configurable?, new error variant, dependency? |

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

Blocked on a human ruling: 04, 06, 07, 10, 15, 25, 36, 38, 39, 48. Each becomes
its own PR once ruled — 36 and 38 are the two large design changes.

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
