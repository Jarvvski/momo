# 09: Rewriting the changelog footer deletes user-authored reference links

Type: bug
Status: resolved
Severity: high
Area: domain/changelog
Verified: yes — read at the cited lines

## Symptom

`parse` puts *every* trailing `[label]: url` line into `footer`. `refresh_footer`
then replaces that entire block with only momo's own `[Unreleased]` / `[x.y.z]`
lines. Standard Keep-a-Changelog footers carry extra definitions that are not
momo's, and they are destroyed on the next release. With no derivable remote the
whole footer — including every version link — is erased.

## Evidence

`src/domain/changelog.rs:152-161`:

```rust
fn refresh_footer(&mut self, links: Links<'_>) {
    self.footer = match links.remote {
        Some(remote) => remote.footer(
            &self.released_versions(),
            links.prefix,
            self.unreleased.is_some(),
        ),
        None => Vec::new(),      // <-- wipes the entire footer
    };
}
```

A footer such as:

```
[Keep a Changelog]: https://keepachangelog.com/en/1.0.0/
[Semantic Versioning]: https://semver.org/spec/v2.0.0.html
[Unreleased]: https://github.com/o/r/compare/v1.0.0...HEAD
```

loses the first two on `momo release`, breaking any prose that references them.

## Trigger

Any changelog with a non-momo reference link in its footer, then `momo release`.

## Suggested direction

Rewrite only the lines momo owns (`[Unreleased]` and the version links), and
preserve every other definition verbatim. When `remote` is `None`, leave the
existing footer alone rather than emptying it.

## Open questions for triage

- Which footer lines does momo consider "its own"? A prefix match on the tag
  prefix is one option; so is a whitelist of the version-link shape.

## Triage

**Verdict:** `ready-for-agent` — real, verified. `refresh_footer`
(`changelog.rs:152-161`) replaces the whole `footer` vec with
`remote.footer(...)` (which emits only momo's `[Unreleased]`/`[x.y.z]` lines) or
`Vec::new()` when there is no remote, so every user-authored definition is
destroyed on the next `release`/`candidate`. Only those two verbs call it;
`record` renders `footer` unchanged, so `momo add` does not trigger this.

**Corrections:**
- The wipe happens in both branches: with a remote, only momo's lines survive;
  without one, the entire footer (including momo's own version links) is
  emptied. The ticket's "loses the first two" and "whole footer erased" are both
  right.
- `split_footer` (`220-235`) only captures a *trailing* block separated by a
  blank line, and only lines containing `]: ` (with the space). A footer without
  that separator stays in the body and is preserved by accident.
- Add: this fires in fragments mode too — `release.rs` always parses and
  re-renders the changelog.

**Scope:** Standalone and low-risk; it is also the footer slice of 38, but does
not depend on it. Not a duplicate of 10/13. Keep.

**Implementation sketch:**
- `src/domain/changelog.rs`: add `fn is_owned_link(line: &str) -> bool` — a
  reference link whose label is `Unreleased`/`[Unreleased]` or parses as a
  semver `Version`. Rewrite `refresh_footer` to merge rather than replace: keep
  every existing footer line that is *not* owned, in order, and re-emit momo's
  owned lines from `remote.footer(...)`. When `links.remote` is `None`, return
  without touching `self.footer` (the ticket's preferred "leave it alone").
- Ordering: simplest is `unowned (original order) ++ regenerated owned`; note the
  small reorder in the commit message.
- Tests (inline `mod tests`): a footer carrying `[Keep a Changelog]:` /
  `[Semantic Versioning]:` plus momo links survives a `release` with a remote
  and has its version links updated; a footer survives a `release` with no
  remote; unit-test `is_owned_link`.

## Resolution

Resolved in the `changelog-fidelity` change set.

`refresh_footer` (`src/domain/changelog.rs`) now merges instead of replacing.
`is_owned_link`/`link_label` decide which footer lines momo owns — the
`[Unreleased]` link and any label that parses as a semver `Version`. Every other
definition is kept, in its original order, ahead of the regenerated owned lines
(a small reorder: user definitions now lead). With `links.remote == None` the
footer is left exactly as it was rather than emptied.

Tests: `is_owned_link_matches_only_momo_reference_links`,
`a_release_preserves_user_authored_footer_links`,
`a_release_without_a_remote_leaves_the_footer_alone`.
