# Recording model: single changelog vs fragments, and how momo chooses

Type: grilling
Status: resolved

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

## Answer

Settled (all recommendations accepted):

1. **Selection:** an explicit config key
   `recording = "changelog" | "fragments"`, default `changelog`, written by
   `momo init`. Not auto-detected — the model is a deliberate, recorded choice.
   (Feeds ticket 04.)
2. **A Fragment** is a file at `.momo/changes/<kind>-<slug>.md`, where
   `kind ∈ break|add|fix` — the Kind is encoded in the filename, so **no
   front-matter** is needed. The body is the entry text (markdown allowed).
   Canonicalised in `GLOSSARY.md`.
3. **Authorship:** `momo break|add|fix` is the canonical writer, but fragments
   are plain files and may be hand-authored or CI-written; `status` picks up any.
4. **Lifecycle:** `status` / `--dry-run` read fragments read-only; `release`
   reads all fragments, writes the versioned changelog section, then **deletes**
   them (consumed exactly once). Precedent: changesets `version`, towncrier and
   scriv `git rm` fragments on build.

Left to ticket 03: whether the changelog keeps an `## Unreleased` section in
fragment mode.

Unblocks ticket 03; feeds ticket 04.
