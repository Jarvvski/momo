# 13: CRLF changelogs are silently rewritten to LF

Type: bug
Status: resolved
Severity: low
Area: domain/changelog
Verified: yes — read at the cited lines

## Symptom

`parse` uses `str::lines()`, which strips `\r` from CRLF, and `render` joins with
`\n`. The first `momo add` or `momo release` in a CRLF-authored changelog rewrites
every line ending, producing a whole-file diff. Trailing blank-line count is
normalised too.

## Evidence

`src/domain/changelog.rs:52`: `let lines: Vec<&str> = text.lines().collect();`
`src/domain/changelog.rs:183-185`:

```rust
let mut rendered = lines.join("\n");
rendered.push('\n');
```

## Trigger

A `CHANGELOG.md` with CRLF line endings (a Windows or legacy checkout), then any
recording command.

## Suggested direction

Detect the dominant line ending on parse and reapply it on render. (Same class of
issue exists for `Cargo.toml` — see ticket 34.)

## Open questions for triage

- Is CRLF support in scope for a tool that is Linux/macOS-only for now? If not,
  this may be `wontfix` with a documented limitation.

## Triage

**Verdict:** `wontfix` — real, verified, but out of scope. momo ships Linux/macOS
only (no Windows target; see README/usage), and normalising CRLF→LF is arguably
desirable; git's `core.autocrlf` is the right place to handle it. This is a
whole-file-diff nuisance, not data loss.

**Corrections:**
- `str::lines()` strips the trailing `\r`, so *parsing* is correct; the loss is
  only on `render` (join with `\n`, `changelog.rs:183`). Trailing blank-line
  count is normalised too.
- Same class as ticket 34 (`Cargo.toml` CRLF via `toml_edit`); rule them
  together.

**Scope:** Close as `wontfix`; document the limitation. If 38 lands the preserve
model, untouched regions would round-trip, but momo's own inserted lines are
still LF — so CRLF is not "fixed for free".

> **Correction (2026-10-14).** 38 did land the preserve model, and the prediction
> above was wrong: momo's own inserted lines are written with the document's own
> ending, so a CRLF changelog round-trips in full. This ticket is `resolved`, not
> `wontfix`. See `## Resolution`.

**Implementation sketch:** none. Add a line under "Not yet supported" in
`docs/usage.md`: CRLF line endings in a changelog (or `Cargo.toml`) are
normalised to LF on the first write; run git with `core.autocrlf=input`.

## Resolution

Fixed by the `changelog-model` change set (ticket 38, ADR 0003) rather than closed
as `wontfix`.

`Changelog` now detects the document's ending once on parse (`Ending::Lf` or
`Ending::Crlf`) and reapplies it to **every** line it writes, not only to the
regions it passes through. A CRLF changelog therefore round-trips in full,
including the headings and bullets momo itself inserts — the triage's "not fixed
for free" prediction was wrong.

`docs/usage.md` records the behaviour positively (user footer definitions and CRLF
"pass through untouched"), and no "Not yet supported" note was added, because
there is no longer a limitation to document.

Test: `a_crlf_changelog_keeps_its_line_endings`, which asserts a momo-inserted
bullet comes back as `- a fix\r\n`.
