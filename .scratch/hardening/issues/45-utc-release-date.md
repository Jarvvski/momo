# 45: The release date is UTC, not the user's local date

Type: judgement
Status: resolved
Severity: low
Area: cli/commands
Verified: yes — read at the cited lines

## Symptom

The changelog date is stamped from `now_utc()`, so a user east of UTC releasing
late in their local day gets yesterday's date. `today()` is also called *inside*
the per-package loop, so a multi-package release straddling UTC midnight can stamp
different packages with different dates.

## Evidence

`src/cli/commands.rs:309-312`:

```rust
fn today() -> Date {
    time::OffsetDateTime::now_utc().date()
}
```

called at `:178` (release) and `:216` (candidate).

## Trigger

A user at UTC+13 releasing at 23:00 local gets the previous day's date in the
changelog.

## Suggested direction

Capture one date before the package loop, and consider the local offset
(`now_local()`) for the user-facing stamp.

## Open questions for triage

- UTC vs local is a real decision: UTC is reproducible across CI and contributors,
  local matches what the user expects. The spec may already have an opinion — check
  `.scratch/momo-v0/spec.md` §changelog. If UTC is deliberate, the only bug is the
  per-package re-read, which should be `wontfix`-for-date + a one-line fix for the
  loop.

## Triage

**Verdict.** UTC is **deliberate**, not an oversight:
`.scratch/momo-v0/research/rust-crates.md:185-200` chooses `time`, UTC, "no extra
features" and says "`local-offset` … would drag in `libc` + `num_threads`; **UTC is
the right default for a changelog anyway**." So the local-vs-UTC half is
**`wontfix`** (keep UTC; document it). The genuine defect is that `today()` is
re-read per package inside the loop (`commands.rs:178` release, `:216` candidate),
so a multi-package release straddling UTC midnight stamps different dates.
**`ready-for-agent`**, scoped to that.

**Corrections.**
- The title ("not the user's local date") implies local is correct; it is not — UTC
  is a documented, deliberate choice. Reframe the ticket around the per-package
  re-read.
- Both `release` and `candidate` carry the same defect.

**Scope.** The UTC-vs-local question → **`wontfix`** + a docs note; the loop re-read
→ this ticket. A one-line fix; optionally split into its own tiny ticket, but
keeping it here is fine.

**Implementation sketch.**
- `src/cli/commands.rs`: hoist `let date = today();` above the
  `for package in packages` loop in `release` and `candidate`; pass `date` into
  `release::release(...)`/`release::candidate(...)`.
- Test: a two-package mise case asserting both changelog headings carry the same
  date (the midnight straddle itself is untestable); a comment explaining why the
  hoist exists.
- Docs: note in `docs/configuration.md` (or `docs/ci.md`) that the changelog date
  is UTC, closing the judgement.

## Resolution

The per-package re-read is fixed: `today()` is hoisted above the package loop in
both `release` and `candidate` (`src/cli/commands.rs`), so one run stamps one
date. `tests/monorepo.rs::a_multi_package_release_stamps_one_date` pins both
headings to the same day.

The local-vs-UTC half is **`wontfix`**: UTC is deliberate
(`.scratch/momo-v0/research/rust-crates.md`), so `today()` keeps `now_utc()`. The
choice is now documented in `docs/configuration.md`.
