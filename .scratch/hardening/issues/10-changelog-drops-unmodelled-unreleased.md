# 10: The changelog drops any Unreleased content it cannot model

Type: bug
Status: resolved
Severity: high
Area: domain/changelog
Verified: yes — read at the cited lines

## Symptom

Parsing an Unreleased body keeps only `- ` bullets that sit under a heading momo
recognises. `Kind::from_heading` knows only `Breaking`/`Added`/`Fixed`, so a
hand-written `### Changed` — the changelog recording model's whole point — has
its bullets dropped. The loss is silent, and it is persisted the next time momo
rewrites the file.

## Evidence

`src/domain/changelog.rs:250-263`:

```rust
for line in body {
    if let Some(heading) = line.strip_prefix("### ") {
        kind = Kind::from_heading(heading);
    } else if let Some(text) = line.strip_prefix("- ")
        && let Some(kind) = kind
    {
        changes.push(Change::new(kind, text.trim()));
    }
}
```

`src/domain/change.rs:36-43` returns `None` for anything but `Breaking`/`Added`/
`Fixed` (its own test asserts `from_heading("Changed") == None`).

## Trigger

Hand-edit `## [Unreleased]` to include:

```
### Changed

- Reworked the parser
```

then run `momo add fix "..."` or `momo release`. `- Reworked the parser` is gone.
HTML comments, `* item` bullets, indented sub-bullets and wrapped continuation
lines are lost the same way.

Because those changes never reach `bump`, a real break/add under `### Changed` is
also absent from the version computation — `status` can report "nothing
unreleased".

## Suggested direction

Either carry unrecognised content through verbatim (store the raw body alongside
the parsed `Change`s), or fail loudly on a heading momo cannot represent.
Silently shrinking the user's changelog is the wrong default.

## Open questions for triage

- Does momo want to support Keep a Changelog's fuller vocabulary (`Changed`,
  `Deprecated`, `Removed`, `Security`), or only its three Kinds? This is a
  product decision — see the design ticket 38.

## Triage

**Verdict:** `ready-for-human` — real, verified, but **subsumed by 38**. The
proper fix is 38's preserve model: carry the raw Unreleased body through
unchanged and edit only the spans momo owns. The product decision is ruled:
**momo keeps its three Kinds** (`Breaking`/`Added`/`Fixed`), locked in spec §6
and issue 03 — momo does **not** adopt Keep a Changelog's fuller vocabulary. So
10 is not "add `Changed`/`Deprecated`/… to `Kind`"; it is "stop discarding what
momo cannot model".

**Corrections:**
- Broaden the symptom. It is not only `- ` bullets under an unrecognised `### `
  heading: `parse` stores only `entries(...)` (`changelog.rs:77`) and `render`
  re-emits only `push_body(...)` (`167-171`), so *everything* else in the
  Unreleased body is lost — prose, HTML comments, `* item` bullets, indented
  sub-bullets, wrapped continuation lines, and any `### ` heading
  `Kind::from_heading` returns `None` for.
- Bullets under a *recognised* heading (`### Added`) **do** reach `bump`; the
  "never reach bump" consequence applies only to dropped content.
- The "fail loudly" alternative is viable but hostile to the
  hand-editable-changelog model; prefer preserve. Record the choice in 38.

**Scope:** Subsumed by 38 — merge; do not land a partial preserve here (it would
fight 38's model). The batch plan's `09 + 10 + 11` PR B should drop 10.

**Implementation sketch:** none standalone; implement as part of 38 (preserve the
raw Unreleased body; on `release`, promote the whole body, not just the parsed
changes).

## Decision (owner, 2026-10-14)

**Subsumed by ticket 38** (preserve the document). Implement there.

## Resolution

Resolved by the `changelog-model` change set (ticket 38, ADR 0003).

The Unreleased body is no longer parsed-and-re-rendered. `record` appends to the
preserved body and `release` moves the whole body into the new dated section, so
content momo cannot model — a `### Changed` heading and its bullets, HTML
comments, `* item` bullets, prose, wrapped lines — is carried through verbatim
instead of being dropped. Only `- ` bullets under the three recognised Kind
headings feed the bump; everything else is preserved but never interpreted.

`has_unreleased` now reports a non-blank Unreleased body of any kind, so
hand-written unmodelled content is visible to the fragments-mode guard and to a
Release rather than silently invisible.

Tests: `an_unrecognised_heading_and_its_bullets_survive_a_release`,
`html_comments_and_star_bullets_survive_a_release`,
`recording_preserves_unmodelled_unreleased_content`,
`has_unreleased_counts_content_momo_cannot_parse` (unit);
`a_recording_verb_preserves_the_document_it_does_not_own` (integration).
