# 14: Fail loudly on an unusable `remote`

**What to build:** The `remote` key added by ticket 12 is parsed into a `Remote`
(forge + base URL) so the Changelog can render its reference-link footer. When
the configured value is **not** a recognisable remote URL, the current behaviour
is to treat it exactly like an unrecognised VCS remote — silently omit the
footer. That is indistinguishable from "no remote configured", so a typo in
`momo.toml` produces a changelog with no links and no complaint.

Make it an error: a `remote` that **is set** but cannot be parsed fails the run
with a clear message naming the key and the offending value. A `remote` that is
**absent** stays fine — the footer is then derived from the VCS, or omitted when
there is none.

Also note in the README (and `docs/mise-task-distribution.md`) that the release
workflow builds **Linux and macOS only** — four Unix targets, no Windows.

**Blocked by:** None (can start immediately)

**Spec:** `.scratch/momo-v0/spec.md` §6

**Status:** ready-for-agent

- [ ] A `remote` value that cannot be parsed as a remote URL makes the run fail, naming the key and the value.
- [ ] An absent `remote` still derives the footer from the VCS, or omits it when there is no VCS.
- [ ] A valid `remote` continues to work, with and without a VCS.
- [ ] The README and the mise playbook state that releases are built for Linux and macOS only.
