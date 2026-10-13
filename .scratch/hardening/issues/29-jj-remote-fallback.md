# 29: jj `remote()` falls back to the first remote, and can return an empty URL

Type: bug
Status: wontfix
Severity: low
Area: adapters/vcs
Verified: yes — read at the cited lines

## Symptom

The port contract says `origin` is preferred. The git adapter returns `None` when
there is no `origin`; the jj adapter silently falls back to whichever remote is
listed first, so the changelog can link to an `upstream`/fork the user does not
control. A line with a trailing space yields `Some("")`, which `derive_remote`
then discards.

## Evidence

`src/adapters/vcs/jj.rs:51-67`:

```rust
for line in output.stdout.lines() {
    let Some((name, url)) = line.split_once(' ') else { continue };
    let url = url.trim().to_owned();
    if name == "origin" { return Ok(Some(url)); }
    first.get_or_insert(url);
}
Ok(first)
```

The existing test `remote_falls_back_to_the_first_remote_and_reports_none_when_empty`
(`jj.rs:192`) pins the fallback as intended.

## Trigger

A repo whose only remote is `upstream` (a fork). The generated changelog links to
the fork rather than the user's own repo — or, with no `origin`, to whatever jj
lists first.

## Suggested direction

Mirror git (origin or `None`), or document the fallback; skip empty URLs.

## Open questions for triage

- Is the fallback deliberate and wanted? If so, this is `wontfix` plus a doc note.

## Triage

**Verdict:** Confirmed as behaviour, but deliberate. `JjVcs::remote`
(`jj.rs:51-67`) falls back to the first remote when `origin` is absent; the test
`remote_falls_back_to_the_first_remote_and_reports_none_when_empty` (`jj.rs:192`)
pins that as intended. The port doc says `origin` is *preferred*, not required
(`src/domain/vcs.rs:43-48`), so a fallback is within contract, and linking to the
only remote (an upstream you cloned from) is usually more useful than no link.

**Corrections:** The empty-URL claim is technically true (`"name ".split_once(' ')`
→ `""`), but harmless and effectively unreachable: `derive_remote` already
discards an empty URL (the parse fails → `None`), and jj does not emit a remote
with an empty URL.

**Scope:** `wontfix` — no code change. Optional: a one-line note in
`docs/usage.md` that jj falls back to the first remote when there is no `origin`.
