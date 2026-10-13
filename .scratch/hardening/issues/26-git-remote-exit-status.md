# 26: The git adapter treats every `remote get-url` failure as "no remote"

Type: bug
Status: resolved
Severity: medium
Area: adapters/vcs + cli/release
Verified: yes — read at the cited lines

## Symptom

`GitVcs::remote` does not check the exit status, so a genuine failure (a corrupt
config, "dubious ownership", a non-repo) is indistinguishable from "origin is
unset". The changelog then ships with no reference links and no signal. The jj
adapter *does* call `require_success`, so the two disagree.

Separately, `derive_remote` discards a successfully-read but unrecognised URL with
`.ok()`, silently omitting links even though the user could fix the remote.

## Evidence

`src/adapters/vcs/git.rs:49-58`:

```rust
fn remote(&self) -> Result<Option<String>, VcsError> {
    // `get-url` exits non-zero when `origin` is not configured; that is not
    // an error here, it just means there is no remote to link to.
    let args = ["remote", "get-url", "origin"];
    let output = self.git(&args)?;          // exit status ignored
    Ok(match output.stdout.trim() {
        "" => None,
        url => Some(url.to_owned()),
    })
}
```

`src/cli/release.rs:246-254`:

```rust
let Some(url) = vcs.remote()? else { return Ok(None); };
Ok(url.parse().ok())        // unparseable URL silently dropped
```

## Trigger

A repo where `git remote get-url origin` fails for a reason other than "no such
remote" (e.g. `fatal: detected dubious ownership`). momo reports success and emits
a changelog with no links.

## Suggested direction

Have the port distinguish "no remote" from "could not read the remote", and warn
(or fail) when a remote exists but cannot be normalised. Note the asymmetry: a
*configured* `remote` that is unusable is a hard error (`config.rs`), but a
VCS-derived one is silently omitted.

## Open questions for triage

- Warn or fail on an unrecognised VCS remote? Warning is friendlier and keeps
  releases moving.

## Triage

**Verdict:** Confirmed. `GitVcs::remote` (`src/adapters/vcs/git.rs:49-58`) ignores
`output.success`, so a genuine failure (dubious ownership, non-repo, corrupt
config) yields empty stdout and reads as `Ok(None)` — indistinguishable from "no
origin". The jj adapter (`jj.rs:51-54`) uses `require_success`, so the two
disagree. `derive_remote` (`src/cli/release.rs:246-254`) separately drops an
unparseable URL with `.ok()`.

**Corrections:** You cannot fix this by checking the exit status of `get-url`
alone: `git remote get-url origin` exits non-zero both when `origin` is unset and
on a real failure. List the remotes first, then read the URL only if `origin`
exists.

**Scope:** Single. The two halves (adapter failure handling; warning on an
unrecognised VCS remote) form one coherent change. Overlaps 31 — both concern
misreading a git failure as a benign absence.

**Implementation sketch:**
- `git.rs::remote`: run `["remote"]` through `require_success`; if no line equals
  `origin`, return `Ok(None)`; otherwise run `["remote", "get-url", "origin"]`
  through `require_success` and return the trimmed URL.
- `derive_remote`: on a parse error, warn `remote `<url>` is not recognised; no
  reference links added` instead of dropping silently (keeps releases moving).
- Tests: a `git.rs` unit test where `git remote` fails → `VcsError::CommandFailed`;
  a `cli/release.rs` test that an unparseable remote warns and still releases.

## Resolution

`GitVcs::remote` now lists remotes with `["remote"]` through `require_success`
first: an absent `origin` returns `Ok(None)`, while a failed listing propagates
`VcsError::CommandFailed`. Only when `origin` exists does it run
`["remote", "get-url", "origin"]`, also through `require_success`. `derive_remote`
(`src/cli/release.rs`) now warns
``warning: remote `<url>` is not recognised; no reference links added`` instead of
dropping an unparseable URL with `.ok()`, and still releases.

Tests: `git.rs` covers the origin read, the no-origin case (proving `get-url` is
not run), a non-`origin` remote, and a failed listing → `CommandFailed`; a
`cli/release.rs` unit test releases with an unrecognised remote and asserts no
link definitions; `tests/release.rs` drives the real binary against a git repo
with a bogus `origin` and asserts the warning reaches stderr while the release
succeeds.
