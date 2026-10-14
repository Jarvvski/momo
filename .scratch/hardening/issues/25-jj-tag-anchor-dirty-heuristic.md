# 25: jj tag anchor is inferred from generic dirtiness

Type: bug
Status: resolved
Severity: high
Area: adapters/vcs
Verified: yes — read at the cited lines; behaviour reproduced by an agent in a scratch jj repo

## Symptom

`create_tag` picks the revision from `is_dirty()`, which is `jj diff --summary`
being non-empty — i.e. *any* change in `@`, not "the release is in `@`". jj
auto-tracks untracked files, so one unrelated stray file flips the anchor and the
tag lands on an empty commit instead of the release commit.

This is the same failure the 0.1.1 "anchor a jj tag to the release commit" fix
addressed (`implementation/15-tag-anchor.md`), with a different trigger.

## Evidence

`src/adapters/vcs/jj.rs:82-89`:

```rust
// ... That is right only when the working copy holds the release; after
// `jj commit`, or above `main` after a merge, `@` is an empty commit ...
let revision = if self.is_dirty()? { "@" } else { "@-" };
let set = ["tag", "set", tag.as_str(), "-r", revision];
```

`is_dirty` at `jj.rs:93-97` is `jj diff --summary`.

## Trigger

After `jj commit -m "release 0.1.0"` (so `@` is the empty working-copy commit and
`@-` is the release), `echo stray > stray.txt` makes `jj diff --summary` print
`A stray.txt`; `momo tag` then tags `@` — an empty commit plus the stray file.

A second trigger: after `jj commit`, run `jj new` (start the next change), then
`momo tag` → `@-` is the previous *empty* commit, not the release.

The round-trip suite has no dirty-but-unrelated case.

## Suggested direction

Decide the anchor from where the release actually is — e.g. tag `@` only when the
diff touches the release files, otherwise `@-` — rather than from generic
dirtiness. Better still, stop inferring and take the revision explicitly.

## Open questions for triage

- What is the robust rule? "Tag `@` when `@`'s diff contains the changelog/version
  files, else `@-`" is heuristic too. Is an explicit `--rev`/config the honest
  answer, given `momo tag` is documented as a separate step after committing?
- Related: `@-` is unsound when `@` is a merge (multi-parent revset).

## Triage

**Verdict:** Confirmed — high. Reproduced in a scratch jj 0.46 repo.
`create_tag` (`src/adapters/vcs/jj.rs:87`) picks `@` vs `@-` from `is_dirty()`,
which is "any change in `@`", not "the release is in `@`". After
`jj commit -m "release 0.1.0"`, `jj diff --summary` is empty; writing an unrelated
`stray.txt` makes it `A stray.txt`, so `momo tag` names `@` — the stray-only
commit. The second trigger is also real: `jj commit` then `jj new` leaves both `@`
and `@-` empty, so the tag lands on the intermediate empty commit. And `@-` is
`parents(@)`, which resolves to several commits at a merge, so
`jj tag set -r @-` would fail there too.

**Corrections:** The suggested direction is right, but the fix cannot live
entirely in the adapter: the correct anchor depends on which paths are "the
release" (a package's changelog and `.momo/version`), which only the CLI knows.
The `Vcs` port therefore changes shape. That is why this is not a one-liner.

**Scope:** No split/merge/duplicate. Related: ticket 41 (the jj anchor test
silently skips without jj) and `.scratch/momo-v0/implementation/15-tag-anchor.md`.

**Ruling on the rule:** take the revision explicitly *and* narrow the default.
- Add `momo tag --rev <REV>` (git: any commit-ish; jj: a revset that must resolve
  to exactly one commit) as the honest escape hatch — the tag target is a user
  decision.
- Default anchor = the commit that actually carries the release: if the
  working-copy commit's diff touches the package's changelog or version file, use
  `@`; otherwise walk parents to the nearest ancestor whose diff does. Error on
  ambiguity (a merge) rather than guessing.
- git is unchanged — `HEAD` is already the committed release.

**Implementation sketch (for whoever implements the approved design):**
- Change `Vcs::create_tag` (`src/domain/vcs.rs:54`) to take an explicit revision,
  e.g. `create_tag(&self, rev: &str, prefix: &str, version: &Version, message: &str)`.
  The CLI (`src/cli/release.rs:107`) computes the default revision from the
  package's paths and passes it through.
- jj: probe `jj diff -r <rev> --summary` for the release paths, walking
  `parents()`.
- Tests: extend `src/adapters/vcs/roundtrip.rs` with (a) the stray-file case and
  (b) the `jj commit` + `jj new` case, both asserting the tag names the commit
  whose diff contains `CHANGELOG.md`. Update spec §8 and `docs/usage.md`.

**Why `ready-for-human`:** the anchor rule and the `--rev` CLI/port surface are a
product decision (explicit override vs inference). Once the owner picks a shape,
this is a straightforward agent task.

## Decision (owner, 2026-10-14)

**Default to `@-`, add `--rev`.** The documented flow is release → commit → tag,
so the release sits at `@-`. Default there and drop the dirtiness heuristic; an
uncommitted release passes `--rev @` explicitly. Thread the revision through
rather than inferring it from the working copy.

## Resolution

Done as decided. `Vcs::create_tag` now takes `rev: Option<&str>`; each adapter
applies its own default when it is `None` — git anchors at `HEAD`, jj at `@-` —
and threads an explicit revision through otherwise. The jj adapter no longer
calls `is_dirty`, so an unrelated stray file can no longer mis-anchor the tag
onto the empty working-copy commit. `momo tag --rev <REV>` plumbs the override
from the CLI through `release::tag` to the port; git accepts a commit-ish, jj a
revset.

The port signature change is the only one: `is_dirty` and `VcsError` are
untouched (the separate error-vocabulary/timeout work is unaffected).

Tests: two jj adapter unit tests via `FakeRunner` (default `@-` with no `diff`
probe scripted, and `Some("@")` naming the working copy); a git adapter test
pinning that `--rev` appends the commit-ish; and round-trip tests covering the
stray-file default (`@-`) and the explicit `--rev @` anchor, plus an
end-to-end `momo tag` after `jj commit` + a stray file. The integration tests
that tagged an uncommitted release now pass `--rev @`, matching the documented
release → commit → tag flow. Spec §8 and `docs/usage.md` state the new anchor
rule and the override.
