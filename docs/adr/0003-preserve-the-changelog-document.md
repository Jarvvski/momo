# Preserve the changelog document; edit only the spans momo owns

`CHANGELOG.md` is a hand-editable Keep-a-Changelog file, but momo parsed it into a struct (`Vec<Change>` plus released lines and a footer) and re-rendered the whole document from that struct. Anything the struct could not represent was silently discarded on the next write: user-authored footer definitions, bullets under an unrecognised `### Changed` heading, HTML comments, `*` bullets, wrapped continuation lines, and CRLF line endings. "Silently shrink the user's changelog" was the default behaviour of a tool whose whole point is to keep a reliable changelog.

momo therefore **preserves the document**. The changelog is modelled as the original lines plus the spans momo owns:

- the **Unreleased body** — recording appends to it, releasing moves it;
- momo's own **reference-link footer lines** (`[Unreleased]`, `[x.y.z]`);
- **newly inserted release headings**.

Everything else passes through byte-for-byte: CRLF, `*` bullets, HTML comments, `### Changed`, and user-authored footer definitions. Recording appends to the Unreleased body; releasing moves the whole body into the new dated section, not just the entries the parser understands. Where an edit cannot be applied without touching foreign content, momo **fails loudly, naming the line** — today, a document with more than one Unreleased heading.

The three-Kind vocabulary (`Breaking`/`Added`/`Fixed`) is unchanged: an unrecognised heading is preserved, never interpreted, so it does not affect the bump. Carrying the raw body is what stops momo discarding what it cannot model.

## Considered Options

- **Fail loudly on any unmodelled content** — rejected: hostile to the hand-editable-changelog model; a single HTML comment would block a release. It survives as the fallback for genuinely ambiguous edits (duplicate Unreleased headings).
- **Keep the parsed round-trip and patch each loss** — rejected: a whack-a-mole of lossy paths (footer, unmodelled body, CRLF, order) that the model keeps re-introducing. It is the bug this ADR removes.

## Consequences

- `Changelog::parse` is now fallible; callers propagate a [`ChangelogError`](../../src/domain/changelog.rs) that names the offending line.
- `Changelog::unreleased` returns the parsed Changes for the bump, while `release` moves the raw body — the two views coexist.
- CRLF is preserved for a uniformly CRLF document (momo writes new lines with the document's ending), so the previous "CRLF is normalised to LF" limitation is gone.
- Blank lines *between* sections are normalised to one; content within a section is untouched.
