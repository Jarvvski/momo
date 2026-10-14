# 38: Stop modelling the changelog as a lossy parse → render round-trip

Type: refactor
Status: resolved
Severity: high
Area: domain/changelog
Verified: yes — the lossy paths are read at the cited lines

## Symptom

The changelog is parsed into a struct and re-rendered from it. Anything the struct
cannot represent is silently discarded on the next write: user-authored footer
definitions (09), Unreleased content under unrecognised headings, HTML comments,
`*` bullets, wrapped lines (10), and CRLF (13). "Silently shrink the user's
changelog" is the default behaviour of a tool whose whole point is to keep a
reliable changelog.

## Evidence

`src/domain/changelog.rs:51-81` (parse → `Vec<Change>` + lines), `:164-186`
(render from the struct), `:250-263` (only `- ` bullets under known headings
survive).

## Suggested direction

Pick one:

- **Preserve:** keep the raw document and edit only the spans momo owns (insert
  the Unreleased section, move entries into a new release, rewrite its own footer
  lines). Everything else passes through untouched.
- **Fail loudly:** detect content momo cannot round-trip and refuse, naming the
  offending line, so the user knows before data is lost.

The first is more work but matches the "hand-editable changelog" model the tool
advertises.

## Open questions for triage

- Which model? "Preserve" is the honest answer for a Keep-a-Changelog file but is
  a real redesign of the changelog module.
- Does momo want to support Keep a Changelog's full vocabulary (`Changed`,
  `Deprecated`, `Removed`, `Security`), or only its three Kinds? That decision
  shapes both this ticket and 10.

## Related

- 09, 10, 11, 12, 13, 14.

## Triage

**Verdict:** `ready-for-human` — real, verified; the umbrella design ticket. This
is a redesign of `src/domain/changelog.rs`, not a bug fix, and needs the owner to
sign off on the model before an agent implements it. Ruled decisions: (1)
**preserve** — keep the raw document and edit only the spans momo owns; (2) the
**three-Kind vocabulary stays** (spec §6, issue 03) — unrecognised headings are
preserved, never interpreted. **Subsumes 09, 10 and 13** (footer, unmodelled
Unreleased content and CRLF are all the same lossy round-trip); **overlaps 11**
(duplicate/out-of-order Unreleased) and **14** (fragments-mode Unreleased).

**Corrections:**
- The evidence is right; add that the round-trip also loses *order* when an
  Unreleased section is not first (11), and that `parse` is currently infallible,
  which is what makes the silent loss possible.
- The "fail loudly" alternative is the fallback for genuinely unrepresentable
  edits, not the primary model.

**Scope:** Design; supersedes 09/10/13 and the duplicate/out-of-order half of 11
— close those once it lands. Keep 12 (record-boundary validation) and 14
(fragments orchestration) separate; both are orthogonal.

**Implementation sketch (design outline for the owner):**
- Represent the changelog as the original text plus owned spans (or raw lines
  with a set of momo-owned line indices), rather than a parsed struct re-rendered
  whole.
- Owned spans: the Unreleased body (record/insert), momo's own footer lines
  (`[Unreleased]`, `[x.y.z]`), and newly inserted release headings. Everything
  else passes through byte-for-byte (CRLF, `*` bullets, HTML comments,
  `### Changed`, user footer definitions).
- Recording appends to the Unreleased body; releasing moves the whole Unreleased
  body (not just the parsed changes) into the new dated section.
- Fallback: if an edit cannot be applied without touching foreign content, fail
  loudly naming the line.
- Update spec §6 (consider an ADR); add round-trip fidelity tests (footer,
  unmodelled body, CRLF, out-of-order).

## Decision (owner, 2026-10-14)

**Preserve the document.** Model the changelog as the original lines plus the
spans momo owns — the Unreleased body, its own footer lines (`[Unreleased]`,
`[x.y.z]`), and newly inserted release headings. Everything else passes through
byte-for-byte: CRLF, `*` bullets, HTML comments, `### Changed`, and user-authored
footer definitions. Recording appends to the Unreleased body; releasing moves the
whole body into the new dated section. Where an edit cannot be applied without
touching foreign content, fail loudly naming the line.

Supersedes 09/10/13 and the duplicate/out-of-order half of 11. Update spec §6
(consider an ADR) and add round-trip fidelity tests.

## Resolution

Resolved in the `changelog-model` change set (ADR 0003).

`Changelog` (`src/domain/changelog.rs`) no longer parses into a struct and
re-renders it. It keeps the document's original lines — a preamble, a list of
`##` sections (each a verbatim heading plus its body lines), and the trailing
reference-link footer — plus the line ending the document uses. Only the spans
momo owns are edited: the Unreleased body (`record` appends to it, `release`
moves it whole into the new dated section), momo's own footer lines
(`is_owned_link`/`link_label`, unchanged from 09), and the release headings momo
inserts. Everything else round-trips byte-for-byte: CRLF, `*` bullets, HTML
comments, `### Changed`, user-authored footer definitions, out-of-order released
sections.

`parse` is now fallible: more than one Unreleased heading is refused with a
`ChangelogError::DuplicateUnreleased { line }` that names the line, rather than
silently splitting the document. This is the "fail loudly" fallback; the three
Kind vocabulary is unchanged (unrecognised headings are preserved, never
interpreted).

Behaviour changes from the old model:
- `parse` returns `Result`; callers (`cli/release.rs`, `cli/recording.rs`)
  propagate the error.
- `unreleased()` returns `Vec<Change>` (owned) instead of a slice — the body is
  parsed on demand, so there is no cached parse to go stale.
- `has_unreleased` now means "the Unreleased body has a non-blank line of any
  kind", not "has parsed entries", so a hand-written `### Changed` body is no
  longer invisible to the fragments-mode guard (14) or to a Release.
- A release inserts the new section above the existing ones (newest first) even
  when the Unreleased section sat below a released section; the released section
  is no longer lost into the preamble.
- `release`/`candidate` move the preserved Unreleased body when one exists; a
  bare `## [Unreleased]` heading falls back to the minted Changes (fragments).
- CRLF is preserved (new lines use the document's ending), so ticket 13's
  "normalised to LF" limitation is gone; the note in `docs/usage.md` is removed.

Tests: 30 unit tests in `src/domain/changelog.rs`, including the new round-trip
fidelity cases `an_unrecognised_heading_and_its_bullets_survive_a_release`,
`html_comments_and_star_bullets_survive_a_release`,
`recording_preserves_unmodelled_unreleased_content`,
`a_crlf_changelog_keeps_its_line_endings`,
`an_out_of_order_unreleased_section_is_promoted_not_lost`,
`duplicate_unreleased_sections_are_refused_naming_the_line` and
`an_empty_unreleased_heading_is_replaced_by_the_release`; integration:
`a_recording_verb_preserves_the_document_it_does_not_own`,
`a_duplicate_unreleased_section_is_refused_naming_the_line`,
`release_preserves_a_hand_written_changelog_body_and_footer`.

Docs: ADR 0003; spec §6 updated; `docs/usage.md` recording-model and
"Not yet supported" sections updated.
