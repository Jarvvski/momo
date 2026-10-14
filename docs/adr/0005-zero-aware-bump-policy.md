# The zero-aware bump policy

momo's bump policy is zero-aware: at MAJOR 0 a `break` and an `add` both bump MINOR and a `fix` bumps PATCH; at MAJOR ≥ 1 a `break` bumps MAJOR, `add` MINOR and `fix` PATCH. The baseline is `0.0.0`, and a package that has never released has no version at all rather than `0.0.0`.

The policy matches Cargo's compatibility boundaries everywhere except `0.0.x`. Cargo treats every `0.0.z` as breaking, so it would read a fix at `0.0.0` as `0.1.0`. momo bumps PATCH: `0.0.0` → `0.0.1`. This is a deliberate divergence, not an oversight — the release-plz docs warn that the same `add`-bumps-minor-at-`0.x` behaviour "violates the Cargo SemVer rules", and momo owns the rule anyway.

`0.0.x` is bootstrap scaffolding, not a compatibility promise: there is no released surface for a `0.0.x` break to break, so treating every `0.0.z` as breaking buys nothing. A fix is a fix, and the patch component is where it belongs. The divergence is bounded to `0.0.x` — the moment a project leaves `0.0.x`, the policy is Cargo's.

A second consequence is owned out loud: below 1.0 a `break` and an `add` collapse to the same MINOR bump, so the *version* alone cannot signal that a pre-1.0 release broke compatibility. The `Breaking` changelog section is the only pre-1.0 break signal, which is one reason the three-Kind vocabulary is asserted rather than configurable.

## Considered Options

- **Follow Cargo exactly (every `0.0.z` is breaking)** — rejected: it bumps `0.0.0` → `0.1.0` for a one-line fix, and the "break" it guards against does not exist at `0.0.x`.
- **Special-case `0.0.x` to bump MINOR for `break`/`add` and MAJOR for `break`** — rejected: that is the Cargo rule again under another name, and it makes the first two releases disagree with every later one for no gain.
- **Never assert it; make the bump configurable** — rejected: the bump is the product. A configurable bump policy is a configurable semver, which is no policy at all.

## Consequences

- The policy is pinned by tests in `src/domain/version.rs`: `Kind::bump` at MAJOR 0 and at MAJOR ≥ 1, and the highest-Kind-wins rule. The asserted-versus-defaulted set lives in [Opinions](../opinions.md).
- `Kind::heading` emits `Breaking` where Keep a Changelog emits `Changed` — the second deliberate deviation, stated on the same page.
- Stating the divergence invites "fix it" issues; this ADR is the answer. The rule is deliberate, and it is recorded rather than re-litigated.
