# 15: `momo tag` must anchor to the commit that has the release

**What to build:** In a jj repository the tag command runs `jj tag set <name>`
with no `-r`, which defaults to `@` — the **working-copy commit**. That is right
when `@` holds the release (you ran `momo release` and have not committed yet),
but wrong whenever `@` is empty:

- after `jj commit`, which leaves `@` as a fresh empty commit, and
- after a pull-request merge, where `@` is an empty commit above `main`.

In both cases the tag lands on a commit with no content, and pushing it publishes
a dangling commit to the remote. This bit momo's own v0.1.0 release: the tag had
to be corrected by hand with `jj tag set -r main --allow-move`.

Fix the anchor: tag `@` when the working copy has changes, and its **parent**
when it does not. The jj adapter already has the primitive it needs —
`is_dirty()` reports whether `@` differs from its parent. The git adapter is
unaffected: `git tag` already names `HEAD`, which is the committed release.

Document the jj rule (spec §8 and the README), because it differs from git's.

**Blocked by:** None (can start immediately)

**Spec:** `.scratch/momo-v0/spec.md` §8

**Status:** ready-for-agent

- [ ] In a jj repo where `@` has changes, `momo tag` names `@`.
- [ ] In a jj repo where `@` is empty (after `jj commit`, or above `main` after a merge), `momo tag` names `@`'s parent.
- [ ] A jj round-trip test proves the tag lands on the commit holding the changelog, not on an empty commit.
- [ ] The git adapter's behaviour is unchanged.
- [ ] Spec §8 and the README state the jj anchoring rule.
