# 08: Bump arithmetic can overflow on an adversarial version

Type: bug
Status: resolved
Severity: low
Area: domain/version
Verified: yes — read at the cited lines

## Symptom

`Bump::apply` and `next_candidate` add one to version components with plain `+`.
The current version comes from a tag or `.momo/version`, both user-controlled and
parsed as `u64`. A component at `u64::MAX` panics in debug builds.

## Evidence

`src/domain/version.rs:32-38`:

```rust
Bump::Patch => Version::new(current.major, current.minor, current.patch + 1),
Bump::Minor => Version::new(current.major, current.minor + 1, 0),
Bump::Major => Version::new(current.major + 1, 0, 0),
```

and `src/domain/version.rs:128`: `.map_or(1, |n| n + 1)`.

## Trigger

A tag `v18446744073709551615.0.0` (or the same in `.momo/version`) followed by a
`break` release. In a **debug** build this panics with `attempt to add with
overflow`; in a **release** build (what `cargo install` ships) overflow checks
are off, so it wraps to `0` and silently mints a bogus version such as `0.0.0`.

## Suggested direction

Use checked/saturating arithmetic and surface a clear error, or bound accepted
versions at parse time.

## Open questions for triage

- Is this worth fixing given it requires a `u64::MAX` component, or is it
  `wontfix` on the grounds that a panic in debug is acceptable for absurd input?

## Triage

**Verdict:** `ready-for-agent` (low priority). Not `wontfix`: the shipped binary
is built in release mode, where Rust disables overflow checks, so `u64::MAX + 1`
does not panic — it wraps to `0` and silently mints a bogus version. Silent
corruption is worse than the debug panic the ticket cites, and the guard is
cheap.

**Corrections:** the ticket says "panics in debug builds"; in release builds it
*wraps silently* (see Trigger). Also `next_candidate`'s `n + 1` (line 128)
overflows the same way for a prerelease counter at `u64::MAX`.

**Scope:** no split. Independent of the other versioning tickets.

**Implementation sketch:**
- `src/domain/version.rs`: `Bump::apply` uses `checked_add` and returns
  `Option<Version>` (`None` = overflow). `next_version`/`release_version`
  already return `Option` for "no changes"; add `VersionError::Overflow` and make
  `release_version` → `Result<Option<Version>, VersionError>` so the two states
  stay distinct.
- `next_candidate`: `checked_add`; saturating to `u64::MAX` would just repeat the
  same candidate, so surface the overflow as an error too.
- Update callers in `src/cli/release.rs` and `src/cli/commands.rs` (they already
  propagate with `?`).
- Tests: `Bump::apply` on a `u64::MAX` component → overflow error, not `0`;
  `next_candidate` with `-rc.18446744073709551615` does not wrap.
- Cheaper alternative if signature churn is unwanted: reject a parsed version
  whose any component is `u64::MAX` at the version sources and the
  changelog-heading parse. Weaker, because `Bump::apply` is public domain API.

## Resolution

Resolved in the `version-guards` change set, following the sketch.

- `Bump::apply` uses `checked_add` and returns `Option<Version>` (`None` on
  overflow); `next_version` and `release_version` return
  `Result<Option<Version>, VersionError>`, so "nothing to release" and "overflow"
  stay distinct; `next_candidate` returns `Result<u64, VersionError>`.
- New `VersionError::Overflow { value }` names the component already at
  `u64::MAX`.
- Callers in `cli/release.rs` and `cli/commands.rs` propagate with `?`; the
  `output.rs` test helper expects the `Result`.
- Tests: `Bump::apply` on a `u64::MAX` component returns `None` and
  `next_version`/`release_version` error; a `-rc.18446744073709551615` counter
  errors instead of wrapping.

Correction confirmed end-to-end: the old `+` wrapped silently to `0` in the
release binary. A `u64::MAX` component now errors and leaves the version file
untouched — no bogus `0.0.0` and no repeated candidate.
