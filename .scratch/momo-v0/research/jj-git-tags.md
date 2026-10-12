# jj & git tag mechanics

Research for ticket 12 (`.scratch/momo-v0/issues/12-jj-git-tags.md`): how a tool
creates and inspects a version tag in (a) a git repo and (b) a jj repo.

**Version context.** jj **0.46.0** (released 2026-10-07) and the `latest` docs;
git 2.5x. Everything below was cross-checked against the installed `jj 0.46.0`
CLI help (read-only) and the official jj docs/changelog.

## TL;DR

- **`jj tag` is a real, first-class command** (since jj **0.35.0**). A tool does
  **not** need to shell out to `git` to tag a jj repo.
- jj tags are **lightweight only**. jj **cannot create annotated or signed
  tags**; that is an open feature request ([jj#7908]).
- A jj repo is **always git-backed**. In a **colocated** workspace jj auto-imports
  and auto-exports refs on every command, so a tag made by `git` shows up in jj
  and vice versa. In a **non-colocated** workspace you must run `jj git import` /
  `jj git export`.
- **Safest cross-VCS approach:** detect the VCS (`.jj` → jj, else `.git` → git)
  and use the **native command** for that VCS (`jj tag set` / `git tag`). For
  annotated/signed tags in a jj repo, tag the underlying git repo directly:
  `git -C "$(jj git root)" tag -a|-s ... && jj git import`.

## 1. jj's tag support

### The `jj tag` command exists

`jj tag` has been a stable command since **0.35.0** (2025-11-05). Subcommands:

| Command | Purpose |
|---|---|
| `jj tag set <NAMES>... [-r <REVSET>] [--allow-move]` | Create/update tags (default target `@`) |
| `jj tag list [NAMES]...` | List tags and their targets |
| `jj tag delete <NAMES>...` | Delete tags |
| `jj tag track <TAG[@REMOTE]>...` | Start tracking remote tags |
| `jj tag untrack <TAG[@REMOTE]>...` | Stop tracking remote tags |

Source: `jj tag --help` (installed 0.46.0) and the CLI reference
<https://docs.jj-vcs.dev/latest/cli-reference/#jj-tag>.

`jj tag set` creates/updates **lightweight** tags; it refuses to move an existing
tag unless `--allow-move` is passed (`jj tag set --help`).

### jj tags are lightweight only — no annotated, no signed

The Git-compatibility doc states plainly:

> **Tags: Partial.** You can check out tagged commits by name (pointed to by
> either annotated or lightweight tags). You can also create lightweight tags,
> but you cannot create annotated tags.

Source: <https://docs.jj-vcs.dev/latest/git-compatibility/#supported-features>.

The changelog for 0.35.0 (when `jj tag set`/`delete` landed) confirms the export
behavior:

> New `jj tag set/delete` commands to create/update/delete tags locally.
> Created/updated tags are currently always exported to Git as **lightweight**
> tags. If you would prefer them to be exported as annotated tags, please give us
> feedback on #7908.

Source: <https://docs.jj-vcs.dev/latest/changelog/> (0.35.0, 2025-11-05).
Also 0.37.0: "Fixed failure on exporting moved/deleted annotated tags to Git.
Moved tags are exported as lightweight tags."

The maintainers have discussed a future `jj git tag` for annotated tags, but it
**does not exist yet**; the issue is still open. Source: [jj#7908].

### Tag tracking, fetching and pushing (stabilized in 0.44.0)

The 0.44.0 release notes (2026-08-05):

> Support for fetching and pushing tags has now been stabilized. Tags can be
> tracked or untracked just like bookmarks. Tracked tags are pushed by default.
> New `jj tag track/untrack` commands ... Note that fetched tags are tracked by
> default. `jj git push --all` now pushes all tags in addition to bookmarks.

Source: <https://docs.jj-vcs.dev/latest/changelog/> (0.44.0).

Commands:

- Fetch tags: `jj git fetch -t <PATTERN>` (or `--tag`), tracked by default.
- Push a tag: `jj git push -t <TAG>` (or `--tag`); `jj git push --all` pushes all
  bookmarks **and** tags; `jj git push --tracked` pushes tracked refs only.
- List/filter remote tags: `jj tag list -a` / `--remote <REMOTE>` / `-t` / `-c`.

Sources: `jj git push --help`, `jj git fetch --help` (0.46.0).

### Tags make their targets immutable

The default immutable-head set is:

```
revset-aliases."immutable_heads()" = builtin_immutable_heads()
# builtin_immutable_heads() = trunk() | tags() | untracked_remote_bookmarks() | untracked_remote_tags()
```

Source: <https://docs.jj-vcs.dev/latest/config/#set-of-immutable-commits>.
Practical effect: creating a tag makes the tagged commit (and its ancestors)
immutable in jj, so `jj edit` etc. refuse without `--ignore-immutable`. This is
the behavior debated in [jj#7908].

### Changelog timeline for jj tags

| jj | Date | Tag-related change |
|---|---|---|
| 0.14.0 | 2024-02-07 | `jj tag list` prints imported git tags |
| 0.34.0 | 2025-10-01 | Git-tracking tags recorded (native tagging groundwork) |
| 0.35.0 | 2025-11-05 | `jj tag set` / `jj tag delete` |
| 0.37.0 | 2026-01-07 | Export of moved/deleted annotated tags fixed (moved → lightweight) |
| 0.38.0 | 2026-02-04 | `jj tag list -r` revset filtering; `remote_tags()` revset |
| 0.44.0 | 2026-08-05 | `jj tag track`/`untrack`; fetch+push tags stabilized |

Source: <https://docs.jj-vcs.dev/latest/changelog/>.

## 2. git tag commands

From `git-tag` (<https://git-scm.com/docs/git-tag>). All commands take an
optional `<commit>` (default `HEAD`).

### Create

| Kind | Command |
|---|---|
| Lightweight (a ref straight to the commit) | `git tag v1.2.3 [<commit>]` |
| Annotated (tag object: tagger, date, message) | `git tag -a v1.2.3 -m "Release 1.2.3" [<commit>]` |
| Signed (GPG/X.509/SSH per `gpg.format`) | `git tag -s v1.2.3 -m "Release 1.2.3"` |
| Signed with a specific key | `git tag -u <key-id> v1.2.3 -m "Release 1.2.3"` |
| Force-move an existing tag | `git tag -f ...` |
| Delete | `git tag -d v1.2.3` |
| Verify a signature | `git tag -v v1.2.3` |

Notes (same source):
- `-m <msg>`, `-F <file>` or `--trailer` **implies `-a`** if none of `-a`/`-s`/`-u`
  is given.
- Annotated/signed tags contain a creation date, tagger name+email, message, and
  optional signature; a lightweight tag is "simply a name for an object".
- Git itself distinguishes intent: *"Annotated tags are meant for release while
  lightweight tags are meant for private or temporary object labels. For this
  reason, some git commands for naming objects (like `git describe`) will ignore
  lightweight tags by default."*
- Re-tagging a pushed tag is discouraged; `-f` overwrites locally but does not
  rewrite what others already fetched.

### Inspect

| Goal | Command |
|---|---|
| List all tags | `git tag` or `git tag -l [<pattern>]` |
| List with annotation lines | `git tag -n[<num>] -l` |
| Tags pointing at an object | `git tag --points-at <object>` |
| Tags containing a commit (descendant targets) | `git tag --contains <commit>` |
| Tags merged into a commit (ancestor targets) | `git tag --merged <commit>` |
| **Annotated vs lightweight** | `git cat-file -t refs/tags/v1.2.3` → `tag` (annotated) or `commit` (lightweight) |
| Same, bulk | `git for-each-ref --format='%(refname:short) %(objecttype)' refs/tags` |
| Show tag + target | `git show v1.2.3` |
| Resolve / existence check | `git rev-parse v1.2.3`; `git rev-parse -q --verify refs/tags/v1.2.3` |
| Verify signature | `git tag -v v1.2.3` |

`%(objecttype)` "The type of the object (blob, tree, commit, tag)" — an annotated
tag's ref points at a `tag` object, a lightweight tag's ref points directly at a
`commit`. Source: <https://git-scm.com/docs/git-for-each-ref>.

## 3. How a tag created outside jj is seen by jj, and vice versa

**A jj repo is always backed by a git repo.** jj currently has only a git backend
(docs: "Jujutsu has two backends ... One of them uses a regular Git repo").

- **Colocated workspace** (default for `jj git init` / `jj git clone`): `.jj/` and
  `.git/` share the working copy. *"Jujutsu will import and export from and to the
  Git repo on every `jj` command automatically."* So:
  - `git tag -a v1.0 -m ...` → visible to jj on the next `jj` command (as a local
    tag; annotated tags are read fine).
  - `jj tag set v1.0` → written into the git repo's `refs/tags/v1.0` as a
    lightweight tag, immediately visible to `git tag`.
- **Non-colocated workspace** (`--no-colocate`, or `git.colocate=false`): the git
  repo is hidden inside `.jj/`. jj will not auto-sync; run
  `jj git import` (git → jj) and `jj git export` (jj → git). A bare `git tag` at
  the working-copy root will not find that repo.
- **Find the underlying git repo regardless of colocation:**
  `jj git root` prints the git directory. The community workaround for running
  git against a jj repo is `git -C "$(jj git root)" ...`.

Sources: <https://docs.jj-vcs.dev/latest/git-compatibility/> (colocated section,
"you can use both `jj` and `git` commands there"); `jj git import --help` /
`jj git export --help` (both "do nothing in colocated workspaces because the
import/export happens automatically"); `jj git root --help`; and [jj#7908]
(workaround `git -C "$(jj git root)" tag -a ... && jj git import`).

**Pushing:** in a colocated repo a tag created by `git tag` is pushed with plain
`git push`; a tag created by `jj tag set` is pushed with `jj git push -t <name>`
(or `--all`). jj's push uses `--force-with-lease`-style safety checks.

## 4. Safest cross-VCS approach for a tagging tool

### Detect the VCS

| Signal | Meaning |
|---|---|
| `.jj/` directory present (walk up) | jj repo (may or may not also have `.git/`) |
| `.git/` (dir or file) present, no `.jj/` | plain git repo |
| Both present | **colocated** jj repo — prefer the jj path |

Programmatic probes:

- jj: `jj workspace root` prints the workspace root and exits non-zero outside a
  repo (`Error: There is no jj repo in "."`). (`jj workspace root --help`.)
- git: `git rev-parse --is-inside-work-tree` prints `true`/`false`;
  `git rev-parse --show-toplevel` prints the work-tree root and errors if there is
  none. Source: <https://git-scm.com/docs/git-rev-parse>.

Detect `.jj` **first**: a colocated repo answers "yes" to both, and using jj keeps
jj's tag tracking and immutability semantics intact.

### Choose the command by VCS

| Repo | Create tag | Inspect | Delete |
|---|---|---|---|
| jj | `jj tag set v1.2.3 -r <revset>` | `jj tag list v1.2.3` / `-r '<rev>::'` | `jj tag delete v1.2.3` |
| git | `git tag v1.2.3 <commit>` (lightweight) or `git tag -a v1.2.3 -m "..." <commit>` | `git tag -l v1.2.3`; `git cat-file -t refs/tags/v1.2.3` | `git tag -d v1.2.3` |

Both refuse to overwrite an existing tag by default (`jj tag set` needs
`--allow-move`; `git tag` needs `-f`), which is the safe default for a release
tool: error on an existing tag rather than silently moving it.

### Annotated/signed tags in a jj repo

`jj tag set` cannot produce them. If momo must create annotated/signed release
tags in a jj repo, write the git tag object into the underlying repo and import:

```sh
# works for colocated and non-colocated jj repos
git -C "$(jj git root)" tag -a v1.2.3 -m "Release 1.2.3" "$(jj log -r <rev> --no-graph -T commit_id)"
jj git import          # make jj aware (no-op to run explicitly if colocated)
# signed variant:
git -C "$(jj git root)" tag -s v1.2.3 -m "Release 1.2.3" "$(jj log -r <rev> --no-graph -T commit_id)"
```

The commit id for the target revision can be read with
`jj log -r <rev> --no-graph -T commit_id`. This pattern is the documented
community workaround in [jj#7908] and matches the `jj git root`-based approach.

### Tag-name validity

Both backends validate names as git refs: git via `git check-ref-format`
(<https://git-scm.com/docs/git-check-ref-format>), jj by the same rules. A release
tool should reject names that are not valid refs before invoking either command.

### Recommendation for momo (v0)

1. Detect `.jj` → treat as jj; else `.git` → git.
2. jj: create with `jj tag set <version> -r <revset>` (lightweight, jj-native).
3. git: create with `git tag -a <version> -m <message> <commit>` (annotated, the
   git-intended release form) — or lightweight if the project prefers.
4. If signed/annotated tags are required in a jj repo, fall back to
   `git -C "$(jj git root)" tag -s ... && jj git import`.
5. Check existence first and error rather than move an existing tag.

## Sources

- jj CLI reference — `jj tag`: <https://docs.jj-vcs.dev/latest/cli-reference/#jj-tag>
- jj Git command table (tag rows): <https://docs.jj-vcs.dev/latest/git-command-table/>
- jj Git compatibility ("Tags: Partial"; colocated import/export):
  <https://docs.jj-vcs.dev/latest/git-compatibility/>
- jj changelog (0.14.0 → 0.44.0 tag changes): <https://docs.jj-vcs.dev/latest/changelog/>
- jj config — set of immutable commits (tags as immutable heads):
  <https://docs.jj-vcs.dev/latest/config/#set-of-immutable-commits>
- jj templates — `Commit.tags()` / `.local_tags()`:
  <https://docs.jj-vcs.dev/latest/templates/#commit-type>
- jj issue #7908 — lightweight vs annotated tags; no `jj git tag` yet; workaround:
  <https://github.com/jj-vcs/jj/issues/7908>
- git-tag: <https://git-scm.com/docs/git-tag>
- git-for-each-ref (`%(objecttype)`): <https://git-scm.com/docs/git-for-each-ref>
- git-rev-parse (`--is-inside-work-tree`, `--show-toplevel`, `--git-dir`):
  <https://git-scm.com/docs/git-rev-parse>
- git-check-ref-format: <https://git-scm.com/docs/git-check-ref-format>
- Installed CLI: `jj 0.46.0` — `jj tag --help`, `jj tag set --help`,
  `jj tag list --help`, `jj git push --help`, `jj git fetch --help`,
  `jj git import --help`, `jj git export --help`, `jj git root --help`,
  `jj workspace root --help`.
