# Recording model: single changelog vs fragments, and how momo chooses

Type: grilling
Status: open

## Question

momo must support **both** recording models:

- **(a)** a single `CHANGELOG.md` with an `## Unreleased` section that
  `momo add|fix|break` appends to; and
- **(b)** fragment files (changesets/towncrier style): one file per change,
  consumed into `CHANGELOG.md` at release.

How does a project **select** which one?

- Config key (e.g. `recording = "file" | "fragments"`)?
- Auto-detect (a fragments directory exists → fragments)?
- Support both at once (read fragments if present, else the file)?

Sub-questions:

- Where do fragments live (`.momo/changes/`? `changes/`?) and what is their
  format (front-matter for the Kind + a body)?
- Is `momo add|fix|break` the only writer, or can files be hand-authored?
- How do `status` and `release` consume each model?
- How do fragments avoid merge conflicts (the reason to want them)?
