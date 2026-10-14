# 38: Stop modelling the changelog as a lossy parse → render round-trip

Type: refactor
Status: ready-for-agent
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
