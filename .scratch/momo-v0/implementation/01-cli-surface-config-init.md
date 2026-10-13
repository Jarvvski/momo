# 01: CLI surface, config, and `init`

**What to build:** The complete command surface and the config layer, so every
later ticket fills in a handler instead of adding flags. `momo --help` lists every
verb — `break`, `add`, `fix`, `status`, `release`, `candidate` (alias `rc`),
`tag`, `init`, `install` — with the short aliases `b`/`a`/`f`, plus the global
flags `--dry-run`, `--format`, `--version-only`, `--output`, `-p`/`--package`,
and `--all`. Verbs that are not yet implemented exit non-zero with a clear
"not implemented" message rather than panicking. `momo init` writes a
`momo.toml`; config resolves CLI flags > `MOMO_*` env vars > `momo.toml` >
built-in defaults, and write commands create the file lazily. `init` touches
nothing but `momo.toml`.

**Blocked by:** None (can start immediately)

**Spec:** `.scratch/momo-v0/spec.md` §3, §12

**Status:** ready-for-agent

- [ ] `momo --help` and each verb's `--help` show the full surface above, including aliases and global flags.
- [ ] `momo init` writes a valid `momo.toml` at the repo root and leaves every other file untouched.
- [ ] Config precedence is observable: a flag beats `MOMO_*`, which beats the file, which beats the defaults.
- [ ] A read-only command run where no `momo.toml` exists creates nothing.
- [ ] Unimplemented verbs exit non-zero with a clear message.
