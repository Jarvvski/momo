# 35: Recording writes are non-atomic, and fragment naming has a TOCTOU race

Type: bug
Status: ready-for-agent
Severity: medium
Area: cli/recording
Verified: yes — read at the cited lines

## Symptom

Changelog mode does read-modify-write on the whole file, so two concurrent
`momo add` invocations lose one change (last writer wins). Fragment mode picks a
free file name with `exists()` then writes, so two concurrent invocations can
choose the same name and one clobbers the other.

## Evidence

`src/cli/recording.rs:48-57` (changelog: read, mutate, `fs::write`)
`src/cli/recording.rs:79-93`:

```rust
let path = dir.join(change.fragment_file_name());
if !path.exists() {
    return path;
}
...
(2u32..)
    .map(|n| dir.join(format!("{stem}-{n}.{extension}")))
    .find(|candidate| !candidate.exists())
```

## Trigger

Two `momo add` calls racing (a plausible CI/agent pattern), or the same text
recorded twice concurrently.

## Suggested direction

Write via a temp file + atomic rename; for fragments, create with
`OpenOptions::create_new` and retry the suffix on `AlreadyExists`.

## Open questions for triage

- Is concurrent recording a real scenario for momo, or is this defensive? If momo
  is always invoked serially, this may be low priority — but the atomic write is
  cheap and also covers crashes mid-write.

## Triage

**Verdict:** ready-for-agent — the concrete fixes (atomic write; fragment
`create_new`) are specified and implementable unattended. One facet is out of
scope.

**Corrections:** The body conflates two failure modes. Temp-file + rename fixes
*torn/partial* writes (crash mid-write) but does **not** fix the changelog-mode
*lost update*: two concurrent read-modify-write cycles still both read the old
file and the last `rename` wins, even though each write is atomic. Genuine
mutual exclusion would need a file lock (`std::fs::File::lock`/`try_lock`, stable
on this toolchain) — a separate decision, and the ticket's own open question
("is concurrent recording real?") is unresolved.

**Scope:** Implement the unambiguous parts now; leave changelog-mode concurrency
out. The atomic-write helper is shared with 37 — build it once
(`src/cli/atomic.rs`).

**Implementation sketch:**
- **Files:** `src/cli/recording.rs`; the shared helper module from 37.
- **Shape:** (a) add `write_atomic(path, contents)` (temp in `path.parent()`,
  then `fs::rename`; clean up the temp on error) and use it for the changelog
  branch (`src/cli/recording.rs:55`) and, if convenient,
  `FileVersionSource::write`. (b) Replace `unique_fragment_path`'s
  `exists()`-then-`fs::write` TOCTOU (`:79-93`, `:65`) with
  `OpenOptions::new().write(true).create_new(true).open(candidate)`, advancing
  the `-N` suffix on `ErrorKind::AlreadyExists`. Keep the `exists()`-based
  preview for `--dry-run` (nothing is written; the race is harmless there).
- **Tests to add:** unit — `write_atomic` replaces content and leaves no temp
  behind; the `create_new` retry picks `-2` when the base name exists.
  Integration (`tests/recording.rs`) — after `add`, no temp artefacts remain; a
  pre-seeded base fragment makes `add` land on `-2` (a deterministic stand-in for
  the race).
- **Out of scope:** mutual exclusion for concurrent changelog-mode writers;
  raise it as its own decision if concurrent `momo add` is a real workflow.
