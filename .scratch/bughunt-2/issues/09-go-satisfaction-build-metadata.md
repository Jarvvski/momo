# 09: Go satisfaction compares build metadata

Type: bug
Status: ready-for-agent
Severity: medium
Area: adapters/manifest/go
Verified: yes — code path confirmed against `semver`'s `Ord`

## Symptom

An `+incompatible` pin is reported unsatisfied by the very version it names,
because the comparison uses `semver::Version`'s total `Ord`, which orders build
metadata, rather than semver precedence, which does not.

## Evidence

`src/adapters/manifest/go.rs:77` uses `*version >= pinned`. `semver::Version`'s
`Ord` compares build metadata, so `2.0.0 < 2.0.0+incompatible`, and
`satisfied("v2.0.0+incompatible", 2.0.0)` returns `Some(false)`. Go treats
`v2.0.0+incompatible` and `v2.0.0` as the same base version, so the edge is
falsely unsatisfied. Reachable through a go.mod that pins a v2+ module without a
`/vN` path. No existing test covers it.

## Fix direction

Compare by precedence: use `semver::Version::cmp_precedence` (or compare
`major`/`minor`/`patch`/`pre` and ignore `build`) instead of `>=`. Add a test for
`satisfied("v2.0.0+incompatible", 2.0.0) == Some(true)`.
