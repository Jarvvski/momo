# Changelog format & entry shape

Type: prototype
Status: resolved
Blocked by: 02, 16

## Question

What exactly does the changelog look like? Build a **rough `CHANGELOG.md`** (and
a sample fragment) to react to, then lock the format.

Cover:

- Heading structure — "Keep a Changelog"'s `Breaking / Added / Fixed`, or momo's
  own set? How do `break` / `add` / `fix` map onto headings?
- The `## Unreleased` section: present always, or only when non-empty?
- Per-release heading: `## [1.2.0] - 2026-10-12`? Timestamp format and placement?
- Entry text: bullet shape, optional ID/link (issue/PR), optional author?
- How RC releases appear (if at all).
- How both recording models (ticket 02) render to this.

## Answer

Settled (all recommendations accepted; **Q6 revised to include reference links**).
Prototype: [`../prototypes/changelog-format/`](../prototypes/changelog-format/)
(`CHANGELOG.md` + `NOTES.md`) — throwaway, retained in the scratch area as the
primary source (this repo can't push a throwaway branch; it's jj-only).

1. **Sections:** `Breaking` / `Added` / `Fixed` — momo's three kinds.
2. **Timestamp:** date-only `YYYY-MM-DD` in the version heading.
3. **Unreleased:** in **changelog mode** the `## Unreleased` section is the source
   (present when non-empty); in **fragment mode** the changelog holds only released
   versions and `momo status` reports unreleased — rendering from fragments would
   rewrite the changelog on every `add`, defeating fragments.
4. **Entry:** `- <text> (#issue)` — optional issue/PR ref; no links or authors.
5. **RCs** get their own `## [<version>-rc.N]` heading.
6. **Reference links: yes.** Version headings become links backed by a footer
   (`[Unreleased]: …/compare/<latest-tag>...HEAD`; `[x.y.z]: …/compare/<prev>...<tag>`).
   The URL is **auto-derived from the VCS remote** (forge-detected — GitHub/GitLab/
   Gitea map) with a **config override**, and the footer is **omitted** when there
   is no remote. In a monorepo the links use the per-package `<slug>-v<version>`
   tags.

Per package (ticket 16): one `CHANGELOG.md` per package.
