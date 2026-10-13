# 16: Only warn about discovery when discovery was needed

**What to build:** Package discovery goes through `mise tasks graph --json`,
which refuses to run unless a config sets `monorepo_root = true`. momo treats
that refusal as "discovery unavailable" and prints a warning — on **every
command**. For the ordinary single-package repository, which is the common case
and which simply is not a monorepo, that warning is a false positive and pure
noise.

Distinguish the two situations:

- **Not a monorepo** (no `monorepo_root = true` in scope): a normal
  single-package repo. Degrade **silently**.
- **A monorepo whose graph could not be read** (configured, but the command
  failed): the user asked for something momo could not deliver. **Warn.**

And when the user explicitly asks for packages — `-p`/`--all` — a failure to
discover them must still be loud, because the request cannot be honoured.

Note that mise's error message is not a stable interface, so do not pattern-match
on it; determine whether a monorepo is configured from the configuration itself.

**Blocked by:** None (can start immediately)

**Spec:** `.scratch/momo-v0/spec.md` §9, §10

**Status:** ready-for-agent

- [ ] A repo with a `mise.toml` but no `monorepo_root` runs with no warning.
- [ ] A repo with `monorepo_root = true` whose graph fails to read warns.
- [ ] `-p`/`--all` on a repo where discovery fails still fails loudly.
- [ ] The README documents when discovery is attempted and what happens when it is unavailable.
