# 11: Go `widen` stages a range rewrite the read-only adapter never writes

Type: bug
Status: resolved
Severity: medium
Area: adapters/manifest/go, docs
Verified: yes — code path confirmed; the docs claim checked against the adapter

## Symptom

The Go adapter is read-only — `render`/`write` report a `SkipReason::NoVersion`
and never edit `go.mod` — but `widen` returns `Some`, so `range_rewrites`
(`src/cli/cascade.rs:125`) stages an edit for a Go dependent. `assemble` puts it
in the plan's `ranges`, and `widened_ranges` reports it, so the JSON `ranges`
array would claim a rewrite that never happened. The docs promise the rewrite
outright, contradicting the adapter's own module doc.

## Evidence

`src/adapters/manifest/go.rs:84-91` (`widen`) returns `Some("v{target}")` for any
readable pin, verified against `satisfied` (always true for `v{target}` at
`target`). `go.rs:43-52` (`render`) ignores the edits. The adapter's module doc
(`go.rs:14-15`) says "The adapter is read-only, so the cascade reports a Go edge
but never rewrites it", while `docs/ci.md:161` says a Go pin `v0.1.0` → `v0.2.0`
*is* rewritten and `docs/usage.md:165` says a Go dependent "is now rewritten
where its form is one momo understands". A false `ranges` report is only
reachable if a Go dependency edge exists; mise did not emit one in a `go.work`
monorepo, so the path is currently latent rather than live.

## Fix direction

Make `widen` return `None` for Go, so an unsatisfied Go edge stays in `cascade`
and no phantom rewrite is reported — matching the read-only module doc. Replace
the `widen` tests (`go.rs:247-267`) with one asserting `widen` refuses, and
correct `docs/ci.md:161` and `docs/usage.md:165` to say a Go pin is reported,
never rewritten.
