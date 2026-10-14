# Refuse Unreleased bullets under a heading momo cannot read

[ADR 0003](0003-preserve-the-changelog-document.md) preserves everything momo cannot model and edits only the spans it owns. That is right for *preservation*, but it left a hole in *minting*: in changelog mode the `## Unreleased` body is the bump source, and `entries()` reads `- ` bullets only under `### Breaking`, `### Added` and `### Fixed`. A bullet under `### Changed` sets no Kind and is dropped from the Bump. `Breaking` is not a Keep-a-Changelog heading at all — KaC's break signals are `Changed` and `Removed` — so an idiomatic KaC user who files a break under `### Changed` gets a silently under-versioned release. Nothing is lost from the file (the body is promoted verbatim); only the version is wrong.

momo therefore **refuses to mint** when the Unreleased body holds bullets under a Keep-a-Changelog classification it cannot read. The rule is deliberately narrow:

- **Allowlist, not an open set.** Exactly `Changed`, `Deprecated`, `Removed`, `Security` — KaC 1.1.0's categories minus the three momo reads. A heading outside that set and outside [`Kind::from_heading`](../../src/domain/change.rs) (say `### Internal`) is not a KaC classification, so it stays foreign — preserved and ignored, the documented escape hatch.
- **Unreleased only.** Released sections are never parsed for entries, so a historical `### Changed` must never error — otherwise every KaC changelog fails on adoption.
- **Bullet required.** A bare heading or a prose-only section does not fire; there is no Change to version.
- **`release`/`candidate` refuse; `status` reports.** A mint is about to derive a version from the body, so it errors. `status` is the read-only "show me" verb: it does not error, but it *surfaces* the unversioned sections in both human and JSON output, so the condition is never silent. `status --check` keeps its existing "nothing unreleased" meaning.

The refusal is a `thiserror` variant (`ChangelogError::UnreadSection`) in the domain, matching the convention that the domain owns matchable errors. `Changelog::unread_sections()` is a third view over the preserved body beside `unreleased()` (the Changes for the bump) and the raw body (`release` moves it verbatim).

## Considered Options

- **Widen `Kind` to the six KaC categories** — rejected: the categories are *descriptive, not severities* (`Changed` can break or not), so there is no honest Bump to derive from one. Replacing one guess with another, at a large blast radius.
- **Warn only** — rejected: invisible to CI, which reads `--format json`.
- **Map the headings silently** (`Changed → Add`) — rejected: interpretation of foreign content, which ADR 0003 forbids, and the worst failure mode — it mints an unseen number and under-versions the exact break at issue.
- **Refuse, narrowly scoped** — chosen: consistent with ADR 0003's "fail loudly as the fallback for genuinely ambiguous edits".

## Consequences

- `Changelog::parse` records each section's document line, so the refusal can name it.
- `ChangelogError::UnreadSection { heading, line, entries }` carries a code (`momo.changelog.unversioned-section`) and a one-line fix, hand-rolled in the shape [ticket 13](https://github.com/Jarvvski/momo/blob/main/.scratch/v1-roadmap/issues/13-teaching-errors.md) will formalise.
- `status`'s JSON gains an additive `unversioned` array on each package, empty in the common case — a minor release.
- The fragments model closes the same hole: a `.md` file in the changes dir that is not a recognised Fragment is an error naming the file and the expected prefixes, rather than being silently filtered out of both the bump and the report. Dotfiles and `README*` are ignored.
- A released `### Changed` section still passes through untouched; only the Unreleased body is inspected.
