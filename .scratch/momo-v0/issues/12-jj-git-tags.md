# jj & git tag mechanics

Type: research
Status: resolved

## Question

How does one create and inspect a version tag in (a) a git repo and (b) a jj
repo?

Specifically:

- jj's current **tag support** — does `jj tag` exist, is it `jj git` interop, or
  must a tool shell out to `git`?
- How a tag created **outside** jj is seen by jj, and vice versa.
- Annotated vs lightweight vs signed tags in git.
- The safest **cross-VCS approach** for a tool that must tag either kind of repo.
- How to detect which VCS a project uses.

Report exact commands and cite sources.

## Answer

Full findings: [`../research/jj-git-tags.md`](../research/jj-git-tags.md).

- **`jj tag` is native and first-class** (since jj 0.35.0):
  `jj tag set | list | delete | track | untrack`. No need to shell out to `git`
  to tag a jj repo.
- **jj tags are lightweight only.** jj cannot create annotated or signed tags
  (open request jj#7908); `jj tag set` exports as a lightweight tag.
- **A jj repo is always git-backed.** Colocated (the default) auto-imports and
  auto-exports refs on every command, so `git tag` and `jj tag set` see each
  other's tags; non-colocated needs `jj git import` / `jj git export`. `jj git
  root` locates the underlying git dir in either case.
- **Detect `.jj` first** (else `.git`); use the native command for that VCS.
- **Cross-VCS approach:** jj → `jj tag set <v> -r <revset>`; git → `git tag -a
  <v> -m …` (annotated is git's intended release form). For annotated/signed tags
  in a jj repo: `git -C "$(jj git root)" tag -a|-s … && jj git import`.
- **Both refuse to overwrite an existing tag by default** (`--allow-move` /
  `-f`) — the safe default for a release tool.
- Note: in jj, creating a tag makes the tagged commit **immutable**.

Unblocks ticket 06 (Release side-effects & tag format).
