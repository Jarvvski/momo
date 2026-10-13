# 03: VCS adapters — the `Vcs` port, git, and jj

**What to build:** A `Vcs` port in `momo-core` and its two implementations in
`momo-vcs`. The port can detect a repository, read the latest tag, create a tag,
and report whether the working copy is dirty. The git adapter creates annotated
tags; the jj adapter detects `.jj` first and uses `jj tag set` (lightweight).
Both refuse to overwrite an existing tag. Shell out to the `git` and `jj` CLIs —
no `git2`.

**Blocked by:** 01

**Spec:** `.scratch/momo-v0/spec.md` §5, §8, §14

**Status:** ready-for-agent

- [ ] A `Vcs` trait in `momo-core` with git and jj implementations in `momo-vcs`, injectable in tests.
- [ ] Detection prefers `.jj` over `.git`.
- [ ] Reading the latest tag yields the version; with no tags it reports that none exists.
- [ ] Creating a tag on git is annotated; on jj it is lightweight.
- [ ] Creating a tag that already exists fails with a clear error.
