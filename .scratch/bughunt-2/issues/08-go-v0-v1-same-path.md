# 08: `v0` and `v1` are treated as different Go module paths

Type: bug
Status: ready-for-agent
Severity: high
Area: adapters/manifest/go, cli/cascade
Verified: yes — code path confirmed against the Go Modules Reference

## Symptom

A Go `require` pin at `v0.x` is reported unsatisfied by a `v1.x` release, and a
`v1.x` pin by a `v0.x` release, even though Go serves both from the same
unsuffixed module path. momo's cascade therefore reports a false unsatisfied
edge (and, with `--sync-ranges`, would stage a rewrite) for an ordinary
`v0`→`v1` upgrade.

## Evidence

`src/adapters/manifest/go.rs:77`: `pinned.major == version.major`. Go's semantic
import versioning only requires a `/vN` suffix from major 2 onward — `v0` and
`v1` share one path — so the comparison is too strict. The module doc
(`go.rs:9-12`) says "a new **major** is a *different module path*", which holds
only for major ≥ 2. The existing tests encode the bug:
`go.rs:232-237` asserts `satisfied("v0.1.0", 1.0.0) == Some(false)`, and
`cascade.rs:541-555` relies on it.

## Fix direction

Satisfied when the target is at least the pin *and* they share a module path:
majors 0 and 1 are the same path, majors ≥ 2 must match exactly. Update the
`go.rs` tests (`v0.1.0` vs `1.0.0` → `Some(true)`; keep `v1.4.0` vs `2.0.0` →
`Some(false)`) and the cascade test at `cascade.rs:541-555`.
