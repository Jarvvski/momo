# 31: Tag names are never validated against git ref rules

Type: bug
Status: resolved
Severity: low
Area: adapters/vcs
Verified: no — reported by an agent; `tags.rs:10-12` and `git.rs:60-71` read as cited

## Symptom

`tag_name` concatenates prefix and version with no validation. A `tag_prefix`
containing a space, `..`, `~`, `^`, `:`, `?`, `*`, `[`, or ending in `.lock`
produces an invalid ref, and the existence check misreads the resulting failure as
"tag does not exist" — so the user gets a confusing `git tag` error instead of a
configuration error. A prefix beginning with `-` is worse: git parses it as
options.

## Evidence

`src/adapters/vcs/tags.rs:10-12`:

```rust
pub(crate) fn tag_name(prefix: &str, version: &Version) -> String {
    format!("{prefix}{version}")
}
```

`src/adapters/vcs/git.rs:63-66`:

```rust
let verify = ["rev-parse", "--quiet", "--verify", reference.as_str()];
if self.git(&verify)?.success { return Err(VcsError::TagAlreadyExists { tag }); }
```

Any non-zero exit — invalid name, broken object store, non-repo — reads as "tag
absent". The research note `.scratch/momo-v0/research/jj-git-tags.md:244-248`
already recommends rejecting non-ref-safe names.

## Trigger

`tag_prefix = "-1."` → `git tag --annotate --message msg -1.2.3` is parsed as
options. `tag_prefix = "v 1"` → an invalid ref, reported as an opaque git error.

## Suggested direction

Validate the computed tag name (mirror `git check-ref-format`) and return a clear
configuration error naming the offending prefix.

## Open questions for triage

- Validate at config-parse time (against the prefix alone) or at tag-creation time
  (against the full name)? The prefix alone cannot catch everything, since the
  version contributes characters too.

## Triage

**Verdict:** Confirmed. `tag_name` (`tags.rs:10-12`) is a bare `format!`;
`tag_prefix` is parsed with `parse_string` and never validated (`config.rs:335`);
and `GitVcs::create_tag` (`git.rs:60-71`) treats any non-zero `rev-parse` as "tag
absent". A `-`-leading prefix (`-1.`) is passed to `git tag` as options; a space
or `..` produces an invalid ref reported as an opaque git error.

**Corrections:** Minor. Validate the **full** computed name, not the prefix alone:
the version (and, in a monorepo, the slug) contributes characters a prefix check
cannot see. A config-time check of the prefix is a nice early error but not
sufficient.

**Scope:** Single, low. Overlaps 26 — both misread a git failure as a benign
absence.

**Implementation sketch:** Add a pure `is_valid_ref_name(name: &str) -> bool` in
`tags.rs` mirroring `git check-ref-format` for a `refs/tags/<name>` ref (non-empty;
no space, `~ ^ : ? * [ \` or control chars; no `..`, no `@{`; no leading `-`; no
trailing `.` or `.lock`; no `//`, no leading/trailing `/`). Call it in both
adapters' `create_tag` before spawning, returning a clear configuration error that
names the offending name. Tests: `v1.2.3` and `cargo-momo-v1.2.3` accepted;
`-1.2.3`, `v 1`, `v1..2`, `v1.lock` rejected.

## Resolution

`src/adapters/vcs/tags.rs` gained `is_valid_ref_name` (the `git check-ref-format`
rules above) and `checked_tag_name`, which validates the full `<prefix><version>`
and returns the new `VcsError::InvalidTagName { name }` — `` `{name}` is not a
valid git tag name; check the package's tag prefix``. Both `GitVcs::create_tag`
and `JjVcs::create_tag` now call `checked_tag_name` before spawning, so an
unusable name fails clearly instead of reaching the VCS.

Note: `git check-ref-format refs/tags/-1.2.3` accepts the full ref, but a leading
`-` is still rejected because the name is passed to `git tag` as an argument and
git would parse it as an option. Tests: `tags.rs` covers accepted and rejected
names; each adapter has a test that an invalid name errors before any command runs
(an empty `FakeRunner` panics if one does).
