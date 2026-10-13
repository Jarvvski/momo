# 33: The mise "graph" is a flat list — dependencies, dedup and roots are ignored

Type: refactor
Status: resolved
Severity: low
Area: adapters/taskenv
Verified: no — reported by an agent; `graph.rs:60-71` read as cited

## Symptom

`discover` maps only `id` and `root` from `mise tasks graph --json`. The
`dependencies` edges are discarded, so the "graph" is a flat list in mise's array
order: no topological ordering, no dedup (a duplicate `id` yields two identical
Packages and `--all` releases twice), and `root` is accepted verbatim (absolute or
`..`-containing paths pass straight into `Package::dir`).

## Evidence

`src/adapters/taskenv/graph.rs:60-71` maps `id`/`root` only; the test fixture
(`:91`) includes `dependencies` that are never read.

## Trigger

A mise graph listing a project twice → `momo release --all` writes its changelog
and version twice and attempts the same tag twice.

## Suggested direction

At minimum dedup by identity and validate roots. If ordering ever matters, build
the DAG from `dependencies` and topo-sort deterministically.

## Open questions for triage

- Docs say there is no cascading in v0, so ordering is not currently a correctness
  bug. Is dedup + root validation enough, or should the graph be modelled properly
  now?

## Triage

**Verdict:** Confirmed as cited. `parse_projects` (`graph.rs:60-71`) maps only
`id` and `root`; `dependencies` is ignored (the fixture at `:91` includes it),
there is no dedup, and `root` goes verbatim into `Package::from_identity` →
`Package::dir`, so an absolute or `..` root escapes the repo.

**Corrections:** "No topological ordering" is accurate but not a correctness bug:
per spec §9 / `docs/usage.md` there is no cascading in v0. The actionable parts are
duplicate identities and unvalidated roots.

**Scope:** Single, low. Reduce scope to dedup + root validation; explicitly defer
building the DAG/topo-sort until cascading exists.

**Implementation sketch:** In `parse_projects`, dedup by `identity` (keep the
first, or error when duplicates disagree on `root`), and reject an absolute or
`..`-containing `root` (mirror the config-path rule from ticket 24). Tests: a
duplicate `id` yields one Package; an absolute `root` is rejected.

## Resolution

Narrowed scope, as triaged. `parse_projects` (`taskenv/graph.rs`) now dedups by
identity — the first occurrence wins, and a duplicate that disagrees on `root` is
an error rather than a silent second Package — and validates each `root` with a
new `validate_root`: an absolute `root`, or one with a `..` component, is
rejected. The `dependencies` edges are still ignored and there is still no
topological sort: per spec §9 / `docs/usage.md` there is no cascading in v0, so
ordering is not a correctness bug.

Tests: `a_duplicate_id_yields_one_package`,
`a_duplicate_id_with_a_different_root_is_rejected`,
`an_absolute_root_is_rejected`, `a_parent_escaping_root_is_rejected`.

