# 02: Refuse to mint from unread changelog sections

Type: bug
Status: resolved
Points: 16
Bucket: Trust & correctness
Severity: high
Area: domain/changelog + cli

## Problem

In changelog mode the `## Unreleased` body is the bump source. `entries()` walks it
and maps each `### ` heading through `Kind::from_heading`, which knows only
`Breaking`/`Added`/`Fixed`. A `### Changed` heading sets `kind = None` and every
`- ` bullet beneath it is **dropped from the bump**. `Breaking` is not a
Keep-a-Changelog heading at all — KaC's break signals are `Changed` and `Removed`
— so an idiomatic KaC user who files a break under `### Changed` gets a silently
under-versioned release. This is a correctness bug in *minting*, not
preservation: `release` still promotes the body verbatim, so nothing is lost but
the version.

**The fragment model has the same hole, and it is worse.** `fragment_kind` does
`name.split_once('-')?.0.parse::<Kind>().ok()`; a file named `changed-*.md`, or a
typo like `addd-*.md`, yields `None` and `fragment_paths` filters it out
entirely — invisible to both the bump and the report. Docs invite hand-authoring
(`docs/usage.md:18`).

## Evidence

- `src/domain/changelog/parse.rs:53-66` (`kind = Kind::from_heading(heading)`,
  bullets dropped when `None`).
- `src/domain/change.rs:36-43` (`from_heading` knows three headings).
- `Changelog::unreleased()` (`src/domain/changelog.rs:202-206`) is the sole bump
  input (`src/cli/commands.rs:138-144`, `src/cli/release/prepare.rs:98-112`).
- `src/cli/recording.rs:198-205` (`fragment_kind`), `:178-195` (`fragment_paths`).
- Documented as a known limitation: `docs/migrating.md:147`.

## Proposal

**Refuse at mint time, narrowly scoped** — option (b), consistent with ADR 0003's
"fail loudly as the fallback for genuinely ambiguous edits" (`:17`).

Rejected alternatives: (a) widen `Kind` to the six KaC categories — the categories
are *descriptive, not severities*, so there is no honest bump mapping
(`Changed` can break or not); large blast radius to replace one guess with
another. (c) warn only — invisible to CI, which reads `--format json`. (d) map
headings silently — interpretation of foreign content, which ADR 0003 forbids,
and the worst failure mode (mints an unseen number, and `Changed → Add`
under-versions the exact break at issue).

The design, precisely:

- **Allowlist:** exactly `Changed`, `Deprecated`, `Removed`, `Security` (KaC 1.1.0
  minus the three momo reads). Not an open set — an unknown heading
  (`### Internal`) stays foreign, preserved and ignored, and is the documented
  escape hatch.
- **Scope:** the **Unreleased body only**. Released sections are never parsed for
  entries, so a historical `### Changed` must never error — otherwise every KaC
  changelog fails on adoption.
- **Trigger:** an allowlisted heading with **≥1 `- ` bullet**. A bare heading or
  prose-only section does not fire.
- **Where:** `release`/`candidate` error (they are about to mint). `status` does
  **not** error — it is the read-only "show me" verb — but must *surface* the
  unversioned sections in both human and JSON output, so the condition is never
  silent. `status --check` keeps its existing "nothing unreleased" meaning.
- **Domain purity:** pure text over the preserved body; the error is a `thiserror`
  variant, matching the convention that the domain owns matchable errors.

**Fragments:** a `.md` file in the changes dir that is not a recognised fragment
is an error naming the file and the expected prefixes; dotfiles and `README*` are
ignored.

Record the refinement in a short ADR (or an amendment paragraph in 0003): *known
but unread KaC classification headings with Unreleased content are refused at mint
time; unknown headings remain preserved and ignored.*

## Surface

```
$ momo release
error[momo.changelog.unversioned-section]: `### Changed` has 2 entries momo
cannot version (line 41 of CHANGELOG.md)
  fix: move them under `### Breaking`/`### Added`/`### Fixed`, or — if they are
       not versioned changes — out of a Keep-a-Changelog heading momo reads

$ momo status --format json
{ "unreleased": true, "packages": [ { "name": "widget", "current": "1.4.2",
  "next": "1.5.0", "changes": [...],
  "unversioned": [ { "heading": "Changed", "line": 41, "entries": 2 } ] } ] }
```

`unversioned` is additive to the pinned `StatusPackage` shape; empty in the common
case. This is a JSON-contract change → a minor release, documented in
`docs/usage.md` and `docs/migrating.md`.

## Implementation sketch

- `src/domain/change.rs`: `pub fn unread_heading(heading: &str) -> bool` + the
  allowlist const, beside `from_heading`.
- `src/domain/changelog.rs`: `Section` gains `line: usize` (set in `parse`);
  `UnreadSection { heading, line, entries }`; `unreleased()` returns a richer
  `Unreleased { changes, unread }` (extend the ADR-0003 "two views" pattern to a
  third); `ChangelogError::UnreadSection`; `Changelog::reject_unread()`.
- `src/domain/changelog/parse.rs`: `unread_sections(section) -> Vec<UnreadSection>`
  mirrors `entries`.
- `src/cli/release/prepare.rs`: call `reject_unread()?` right after the
  fragments-mode guard, before any write.
- `src/cli/commands.rs::status`: push `unversioned` into `StatusPackage`.
- `src/cli/output.rs`: add the field to the `Serialize` shape and `human()`.
- `src/cli/recording.rs`: three-way `enum FragmentEntry { Fragment(Kind), Foreign,
  Unrecognised(String) }`.
- Tests: `### Changed` with a bullet → one `UnreadSection` at the right line; a
  released-section `### Changed` → none; unknown heading → none; `reject_unread`
  errs; `changed-*.md` fragment errors while `README.md` does not.

## Scoring

Points **16** = Value 5×2 + Ease 3 + Novelty 3. Highest value in the set (it fixes
a silent under-version in the one genuinely unique feature, and closes the same
hole in fragments); moderate effort; the refusal *stance* is novel.

## Risks

- **Fail-loudly-blocks-a-release** (the ADR-0003 trap). Bounded by Unreleased-only,
  allowlist-only, bullet-required, and a one-line fix. If it proves too sharp, add
  an opt-out — do not flip the default.
- **JSON contract change.** Additive, but update the pinned tests; ship as a minor.
- **Fragment strictness is a behaviour change.** Mitigated by the dotfile/`README`
  exemption and a clear message; needs a migration note.
- **Do not infer** whether a `Changed` entry is breaking. That is the rejected
  guess.

## Related

- ADR 0003 (preserve the changelog; fail loudly on ambiguous edits).
- 13 (teaching errors) — ideally this refusal arrives already carrying a code; if
  13 is not yet landed, hand-roll the same `code`/`fix` shape.
- `docs/migrating.md:147`, `docs/usage.md:18`.
