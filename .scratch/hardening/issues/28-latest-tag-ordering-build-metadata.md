# 28: Latest-tag selection uses `Ord` (build metadata), not semver precedence

Type: bug
Status: ready-for-agent
Severity: low
Area: adapters/vcs
Verified: no — reported by an agent; the code path is `tags.rs:22-29`

## Symptom

`latest_version` picks the max with `Version`'s derived `Ord`. `semver::Version`
derives `Ord` over build metadata, which is *not* semver precedence. A repo
carrying both `v1.0.0` and `v1.0.0+build` can select the latter, and that build
metadata then flows into `current` and is discarded by the next bump.

## Evidence

`src/adapters/vcs/tags.rs:22-29`:

```rust
pub(crate) fn latest_version<'a>(
    tags: impl IntoIterator<Item = &'a str>,
    prefix: &str,
) -> Option<Version> {
    tags.into_iter()
        .filter_map(|tag| parse_tag(tag, prefix))
        .max()
}
```

## Trigger

Tags `v1.0.0` and `v1.0.0+meta` present; `latest_tag` returns `1.0.0+meta`.

## Suggested direction

Rank with `Version::cmp_precedence` and a deliberate tie-break, rather than `max`.

## Open questions for triage

- Does momo ever create a tag with build metadata? If not, this is theoretical and
  may be `wontfix` — but it is a one-line hardening.

## Triage

**Verdict:** Confirmed claim. `semver::Version` 1.0.28 derives `Ord` over
`major, minor, patch, pre, build` (`src/lib.rs:157-164`), so `1.0.0+meta` sorts
above `1.0.0`; `latest_version` uses `.max()` (`tags.rs:26-28`) and its own doc
comment claims "Ordering is semver's". `cmp_precedence` (`lib.rs:466`) is the
correct comparison.

**Corrections:** The open question is answered: momo *can* create such a tag —
`momo init --version 1.0.0+build` writes it into `.momo/version`, and `momo tag`
then creates `v1.0.0+build`. The impact is still small: every bump path
(`Bump::apply`, `candidate_version`, `release_version`) drops build metadata, so
the misordering only affects the displayed `current` and the tie-break, never the
version momo mints. It is a real doc-vs-code mismatch, so the one-liner is worth
taking.

**Scope:** Single, low. Keep.

**Implementation sketch:** Replace `.max()` with
`.max_by(|a, b| a.cmp_precedence(b).then_with(|| b.build.cmp(&a.build)))` — on a
precedence tie, prefer the tag with empty build metadata — and fix the doc
comment. Tests: `["v1.0.0", "v1.0.0+meta"]` → `1.0.0`; keep the existing ordering
tests.
