# 17: Do not link `[Unreleased]` when there is no Unreleased section

**What to build:** The changelog's reference-link footer always defines an
`[Unreleased]` link, even when the document has no `## [Unreleased]` heading to
reference. Immediately after a release that is exactly the state — the section
was promoted to `## [x.y.z] - DATE` and nothing is Unreleased — so the footer
carries a dangling link definition:

```markdown
[Unreleased]: https://github.com/Jarvvski/momo/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Jarvvski/momo/releases/tag/v0.1.0
```

Emit the `[Unreleased]` link only when the document actually has an Unreleased
section, and keep emitting a link for every released version. Seen on momo's own
v0.1.0 changelog.

**Blocked by:** None (can start immediately)

**Spec:** `.scratch/momo-v0/spec.md` §6

**Status:** resolved

- [x] A changelog with no Unreleased section has no `[Unreleased]` link definition.
- [x] A changelog with unreleased changes still links `[Unreleased]`.
- [x] Every released version still gets its link, in newest-first order.
- [x] The existing reference-link tests are updated rather than deleted.
