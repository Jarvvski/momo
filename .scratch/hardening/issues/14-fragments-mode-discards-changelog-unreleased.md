# 14: In fragments mode a release discards a changelog-resident Unreleased section

Type: bug
Status: resolved
Severity: medium
Area: cli/release + domain/changelog
Verified: yes — read at the cited lines

## Symptom

In fragments mode, `release` reads the changes from the fragment store but parses
the changelog from disk, then calls `changelog.release(...)`, which sets
`unreleased = None`. Any hand-written `## [Unreleased]` entries in the changelog
are therefore deleted without ever being promoted.

## Evidence

`src/cli/release.rs:138`: `let changes = recording::unreleased(repo_root, config, package)?;` (fragments)
`src/cli/release.rs:146`: `let mut changelog = read_changelog(repo_root, config, package)?;` (file)
`src/domain/changelog.rs:103-107`:

```rust
pub fn release(&mut self, version: &Version, date: Date, changes: &[Change], links: Links<'_>) {
    self.write_section(version, date, changes);
    self.unreleased = None;      // <-- the file's own Unreleased entries are dropped
    self.refresh_footer(links);
}
```

## Trigger

A repo on `recording = "fragments"` that also has a hand-written `## [Unreleased]`
section (e.g. after switching recording models, or a mixed repo). Run
`momo release`; the changelog's Unreleased entries vanish.

## Suggested direction

Detect a changelog-resident Unreleased section while in fragments mode and either
include its entries in the release or refuse with a clear message.

## Open questions for triage

- Is mixing the two recording models a supported state at all? If not, detecting
  and erroring is enough.

## Triage

**Verdict:** `ready-for-agent` — real, verified. In fragments mode `run` takes
`changes` from the fragment store (`release.rs:138`) but parses the changelog
from disk (`146`) and calls `changelog.release(...)`, which sets
`unreleased = None` (`changelog.rs:105`) — any changelog-resident Unreleased
section is dropped without promotion.

**Corrections:**
- Only `Mode::Release` drops the section; `Mode::Candidate` keeps `unreleased`
  but re-renders it from parsed changes, so unmodelled content is still lost
  (see 10).
- In fragments mode the changelog is meant to hold only releases (spec §6), so
  this is a mixed/unsupported state; the fix is to detect and refuse, not merge.
- The fragment path and the file path are the two separate reads at
  `release.rs:138` and `:146` — confirmed.

**Scope:** Standalone. Ruled: refuse with a clear message (don't silently drop,
don't merge). Orthogonal to 38.

**Implementation sketch:**
- `src/domain/changelog.rs`: add `pub fn has_unreleased(&self) -> bool` (or
  expose whether a non-empty Unreleased section was parsed).
- `src/cli/release.rs::run`: after `read_changelog`, if
  `config.recording == Recording::Fragments && changelog.has_unreleased()`,
  `bail!` naming the changelog path and the fix (`move the entries into
  fragments, or set recording = "changelog"`).
- Tests (`release.rs` `mod tests`): fragments config + a changelog with
  `## [Unreleased]` and a bullet → `release` errors and writes nothing; fragments
  config + no Unreleased section → succeeds (existing tests).

## Resolution

Resolved in the `changelog-fidelity` change set.

`Changelog::has_unreleased` reports a *non-empty* Unreleased section, and `run`
(`src/cli/release.rs`) refuses with a message naming the changelog and the fix
when `recording = "fragments"` and the parsed changelog has one. An empty
`## [Unreleased]` heading (the fresh-template case) is left alone, so switching a
repo to fragments does not block every release. Nothing is written and no
Fragment is consumed when the guard fires.

Tests: `has_unreleased_reports_a_non_empty_unreleased_section`,
`a_fragments_release_refuses_a_changelog_resident_unreleased_section`,
`a_fragments_release_ignores_an_empty_changelog_unreleased_heading`;
integration: `release_in_fragments_mode_refuses_a_changelog_unreleased_section`.
