# 05: Monorepo discovery and targeting

**What to build:** Package discovery via `mise tasks graph --json` (which
requires `monorepo_root = true`), per-Package Changelog and Fragment storage,
`-p`/`--package` and `--all` targeting, and per-Package tags named
`<slug>-v<version>`. A Package's identity is its ecosystem ID with a derived,
ref-safe slug, overridable in config. No cascading to dependents.

**Blocked by:** 04

**Spec:** `.scratch/momo-v0/spec.md` §9

**Status:** resolved

- [x] In a repo with several Packages, `momo status` reports each Package and its next version.
- [x] `-p <package>` scopes a command to one Package; `--all` covers every Package.
- [x] Tags are created per Package as `<slug>-v<version>`.
- [x] A single-package repo behaves exactly as it did before this ticket.
- [x] Discovery failure (no `monorepo_root`) degrades to single-Package behaviour with a warning.
