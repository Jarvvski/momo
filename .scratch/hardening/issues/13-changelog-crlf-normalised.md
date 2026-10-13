# 13: CRLF changelogs are silently rewritten to LF

Type: bug
Status: wontfix
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

**Implementation sketch:** none. Add a line under "Not yet supported" in
`docs/usage.md`: CRLF line endings in a changelog (or `Cargo.toml`) are
normalised to LF on the first write; run git with `core.autocrlf=input`.
