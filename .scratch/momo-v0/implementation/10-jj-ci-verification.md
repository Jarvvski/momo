# 10: Verify jj in CI

**What to build:** CI installs only the Rust toolchain, so it has no `jj`. The jj
round-trip integration test in `momo-vcs` therefore skips on CI and jj support
goes unverified there — a gap for a project that must support jj. Pin `jj` in
`mise.toml` so `mise run check` has it, and make the jj round-trip test
**hermetic**: a scratch repository has no `user.name`/`user.email`, so the test
must set an identity itself rather than relying on the runner's global config.

**Blocked by:** 03

**Spec:** infrastructure — closes a verification gap found while landing §8

**Status:** ready-for-agent

- [ ] `jj` is pinned in `mise.toml` and available to `mise run check`.
- [ ] The jj round-trip test actually runs on CI (it must not skip).
- [ ] The test sets its own jj identity, so it passes on a runner with no global jj/git config.
- [ ] `mise run check` is green on CI with `jj` present.
