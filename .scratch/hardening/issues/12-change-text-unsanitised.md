# 12: Change text is unsanitised and injected verbatim into the changelog

Type: bug
Status: resolved
Severity: medium
Area: cli + domain/changelog
Verified: yes — read at the cited lines

## Symptom

The text passed to `momo break|add|fix` reaches the changelog and the fragment
file with no validation. A newline or a Markdown heading in it corrupts the
document; empty text is accepted and still mints a release.

## Evidence

`src/cli/commands.rs:112`: `let change = Change::new(kind, text);`
`src/domain/changelog.rs:278`: `format!("- {}", change.text)`
`src/cli/recording.rs:74-76`: `format!("{}\n", change.text)`

## Trigger

- `momo add $'x\n## [9.9.9] - 2000-01-01'` writes a fake *released* section into
  the changelog. On the next parse that `## ` line is treated as a section
  boundary, so the injected line and everything after it are dropped and
  `released_versions()` is polluted (which also affects candidate numbering and
  the footer).
- `momo add "- nested"` produces a nested list line.
- `momo add ""` records an empty Change, and `release_version` sees a non-empty
  set, so `momo release` mints a version with a `- ` entry.
- A multi-line fragment (`.momo/changes/add-x.md`) is read back with
  `text.trim()` and rendered as raw multi-line text, splitting the list.

## Suggested direction

Normalise or reject text at `record`: reject empty/whitespace-only, collapse or
reject embedded newlines, and neutralise a leading `-`/`#`. Normalise the same way
when reading fragments.

## Open questions for triage

- Collapse internal newlines to a space, or reject multi-line input outright?
- Should duplicates be de-duplicated, or is recording the same text twice a
  legitimate user choice?

## Triage

**Verdict:** `ready-for-agent` — real, verified. Text reaches `Change::new`
(`commands.rs:112`) and is emitted verbatim at `changelog.rs:278` and
`recording.rs:74-76`. Embedded newlines inject arbitrary Markdown (including a
fake `## [9.9.9]` released section, which then pollutes `released_versions()`,
candidate numbering and the footer), and empty text still mints a release
because `release_version` only checks `changes.is_empty()` (`version.rs:76`).

**Corrections:**
- The leading-`-`/`#` cases are minor: the text is always prefixed with `- `, so
  a leading `#` becomes `- # …` (a bullet, not a heading) and a leading `-`
  becomes `- - …`. The real vector is embedded newlines; keep the fix centred on
  that.
- `momo add ""` mints because the recorded `Change` (empty text) is a non-empty
  slice — confirmed.
- Multi-line fragments are read back with `text.trim()` (`recording.rs:128`) and
  rendered raw — confirmed.

**Scope:** Standalone. Ruled: **normalise** rather than reject multi-line
(collapse whitespace runs to a single space) and **reject** empty/whitespace-only.
Apply the same normalisation when reading fragments.

**Implementation sketch:**
- `src/domain/change.rs`: add a validated constructor — `Change::try_new(kind,
  text) -> Result<Change, ChangeError>`, `ChangeError` a `thiserror` enum with an
  `Empty` variant (or a `TryFrom` impl, per house style). It trims, collapses
  every run of whitespace (`\n`/`\r`/tabs included) to one space, and errors when
  the result is empty.
- `src/cli/commands.rs::record`: build via `try_new`, propagating the error with
  a message naming the verb.
- `src/cli/recording.rs::read_fragments`: use `try_new`; decide whether an
  empty/whitespace-only fragment is skipped (friendlier for hand-authored files)
  or surfaced as an error.
- Tests: empty and whitespace-only rejected; `"a\nb"` → `"a b"`;
  `"x\n## [9.9.9] - 2000-01-01"` renders one bullet and no section; a multi-line
  fragment is collapsed.

## Resolution

Resolved in the `changelog-fidelity` change set.

`Change::try_new` (`src/domain/change.rs`) trims the text and collapses every run
of whitespace — newlines and tabs included — to a single space, rejecting
whitespace-only text with `EmptyChange`. `momo break|add|fix` builds the Change
through it (`src/cli/commands.rs`), so a newline can no longer forge a released
section and `momo add ""` no longer mints a Release; the config is not scaffolded
on rejection either. Fragment reads (`src/cli/recording.rs`) normalise the same
way; a Fragment that is empty once normalised is skipped rather than failing the
run. `Change::new` remains the verbatim constructor the parser uses for text it
recovers from an existing changelog.

Tests: `try_new_rejects_empty_or_whitespace_only_text`,
`try_new_trims_and_collapses_whitespace_runs`,
`try_new_flattens_an_injected_section_heading_onto_one_line`,
`a_change_cannot_forge_a_released_section`,
`a_multiline_fragment_is_collapsed_into_one_entry`,
`an_empty_fragment_records_no_change`; integration:
`a_change_verb_rejects_empty_text`,
`a_change_verb_flattens_an_injected_section_heading`.
