# 12: Config, CLI and output conformance

**What to build:** Six places where the shipped binary and the spec disagree.
Fix the behaviour, then make the spec say what is true.

1. **Drop the decorative flag layer.** Spec §12 promised
   `flags > MOMO_* > file > defaults`, but no CLI flag maps to a config key, so
   the top layer never fires. Simplify the chain to
   **`MOMO_*` > `momo.toml` > defaults** — in `Config::resolve`, in spec §12, and
   in the README's precedence section.
2. **Make the changelog footer's remote config-overridable.** Spec §6 says the
   reference-link remote is config-overridable, but §12 has no key for it. Add a
   `remote` key to `momo.toml`: it overrides the remote derived from the VCS, and
   supplies one when there is no VCS. Omit the footer when neither is available.
3. **Let `init` set a start version.** Spec §4 says `momo init` "may set an
   explicit start", but shipped `init` writes only `momo.toml`, leaving a fresh
   repo at `0.0.0` until its first Release. Add `momo init --version <x.y.z>`,
   which writes `.momo/version`; a following `momo tag` then creates the initial
   tag. Validate the argument as a semver version.
4. **Reject output flags where they mean nothing.** `break`/`add`/`fix`, `init`
   and `install` currently accept `--format`, `--version-only` and `--output` and
   silently ignore them. Error clearly instead, naming the flag and the verb.
   The reporting verbs (`status`, `release`, `candidate`, `tag`) keep working as
   they do now.
5. **`Outcome.version_file` becomes a plain `PathBuf`.** Both `release` and
   `candidate` always advance the version file, so the `Option` is always `Some`
   and the JSON contract's `null` case is unreachable. Remove the `Option` and
   the dead null handling.
6. **Amend spec §9 for `tag`'s default scope.** `tag` defaults to every Package,
   symmetric with `release`; §9 only pins `status`/`release`. State it.

**Blocked by:** None (can start immediately)

**Spec:** `.scratch/momo-v0/spec.md` §4, §6, §9, §12, §13

**Status:** resolved

- [x] Config precedence is `MOMO_*` > `momo.toml` > defaults, and the flag layer is gone from the code, spec §12 and the README.
- [x] A `remote` key in `momo.toml` overrides the footer's remote, and supplies one when there is no VCS.
- [x] `momo init --version 1.2.3` writes `.momo/version` holding `1.2.3`; a bad version is rejected.
- [x] `--format`, `--version-only` and `--output` on `break`/`add`/`fix`/`init`/`install` exit non-zero with a clear message naming the flag.
- [x] `Outcome.version_file` is a `PathBuf`, and the JSON contract no longer claims it can be null.
- [x] Spec §4, §6, §9 and §12 match the shipped behaviour.
