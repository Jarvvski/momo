# 13: Release workflow so the mise wrappers resolve

**What to build:** `momo install` writes `momo:*` task wrappers that pin the
running CLI as `#MISE tools={"github:Jarvvski/momo" = "<version>"}`. That pin
resolves to a **GitHub release asset**, and this repository has never published
one — so `mise run momo:status` fails with a 404. The integration is specified,
implemented and documented, but cannot actually run.

Add a release workflow that makes it runnable:

- On a version tag, build the `momo` binary for the supported targets and attach
  archives to a GitHub release.
- **Get the asset naming right.** mise's `github:` backend has a specific
  expectation for how release assets are named per platform and architecture.
  Research it (the mise docs and the `mise-tasks`/`github` backend source are the
  authority) and state the exact expected names in the PR body — do not guess and
  do not assume a generic `momo-<version>-<target>.tar.gz` will resolve.
- Exercise the workflow as far as is possible **without publishing a release** —
  a build job that runs on a pull request, or a `workflow_dispatch` dry run —
  and show that it produces correctly named assets.
- Update `docs/mise-task-distribution.md` to record the release prerequisite: the
  wrappers are inert until a release exists.

**Do not cut a release or create a tag.** `AGENTS.md` reserves releases and tags
for the owner. Build and verify the workflow; the owner triggers it.

**Blocked by:** None (can start immediately)

**Spec:** `.scratch/momo-v0/spec.md` §10 · playbook: `docs/mise-task-distribution.md`

**Status:** ready-for-agent

- [ ] A workflow builds release binaries on a version tag.
- [ ] Asset names match what mise's `github:` backend expects, evidenced from mise's own documentation or source rather than assumed.
- [ ] The build is exercised without publishing (a PR-triggered or dispatched dry run), and the artefacts are shown to be correctly named.
- [ ] `docs/mise-task-distribution.md` records that the wrappers need a published release.
- [ ] No tag is created and no release is published.
