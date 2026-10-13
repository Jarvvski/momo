# Changelog format — prototype notes (ticket 03)

A **rough** take to react to. Not production; the validated decisions move into
`spec.md` and this prototype is discarded.

## The artifact

`CHANGELOG.md` in this directory is a sample **per-package** changelog (ticket
16: one changelog per package) showing both an `Unreleased` section and released
versions, including an RC.

A **fragment** in this format would be a bare file whose *path* carries the kind
and package (ticket 02), e.g.:

```
.momo/changes/momo/add-status-json.md
```

whose contents are just the entry text:

```
`momo status --format json` for CI consumers. (#150)
```

## Variants to react to

1. **Section vocabulary.** Shown: `Breaking` / `Added` / `Fixed` (momo's three
   nouns). Alternative: Keep a Changelog's fuller set
   (`Added`/`Changed`/`Deprecated`/`Removed`/`Fixed`/`Security`).
2. **Timestamp.** Shown: date-only `YYYY-MM-DD` (Keep a Changelog convention).
   Alternative: a full ISO-8601 timestamp.
3. **`Unreleased` section.** Shown: always present. Alternatives: only when
   non-empty; or **absent in fragment mode** (fragments *are* the unreleased
   state, so `momo status` reports them and the changelog holds only releases).
4. **Entry format.** Shown: `- <text> (#issue)`. Alternatives: no issue ref; a
   link; an optional author.
5. **RC entries.** Shown: RCs appear as their own `## [1.2.0-rc.1]` headings.
   Alternative: RCs are omitted from the changelog and only exist as tags.
6. **Reference links.** Not shown. Alternative: Keep a Changelog's
   `[1.2.0]: https://…/compare/…` reference links at the bottom.
