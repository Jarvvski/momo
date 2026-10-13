# 21: `tag_kind` is dead configuration

Type: bug
Status: resolved
Severity: medium
Area: domain/config + adapters/vcs
Verified: yes — grepped every reference in `src/`

## Symptom

`tag_kind` is parsed, merged, serialised into `momo.toml` by `init`, and
documented in `docs/configuration.md` as `lightweight`/`annotated`/`auto` — but
nothing ever reads it. `create_tag` has no kind parameter, so a git user setting
`lightweight` still gets an annotated tag, and a jj user setting `annotated` gets
a lightweight tag with no warning.

## Evidence

`src/domain/config.rs:149` `pub tag_kind: TagKind,` — the only uses in `src/` are
config plumbing (`:171`, `:205`, `:234`, `:311`, `:336`) and one assertion at
`:512`. The `Vcs` port is:

```rust
fn create_tag(&self, prefix: &str, version: &Version, message: &str) -> Result<(), VcsError>;
```

`src/cli/release.rs:123` calls it with no kind. The git adapter always uses
`--annotate` (`src/adapters/vcs/git.rs:68`).

## Trigger

`tag_kind = "lightweight"` in `momo.toml`, then `momo tag` in a git repo — the
tag is annotated.

## Suggested direction

Thread `TagKind` into `create_tag` (and honour `auto` per VCS), or remove the key
from the config surface and the docs. A documented-but-inert key is worse than no
key.

## Open questions for triage

- Implement or remove? Implementing means jj must reject `annotated` explicitly
  (it cannot create them), which is a small extra behaviour.

## Triage

**Verdict:** Confirmed inert. `tag_kind` appears only in config plumbing
(`config.rs:73,83,99,149,171,205-206,234,277-278,311,336`) and one assertion
(`:512`). `Vcs::create_tag` (`domain/vcs.rs:54`) has no kind parameter; git
hardcodes `--annotate` (`adapters/vcs/git.rs:68`) and jj hardcodes a lightweight
`jj tag set` (`adapters/vcs/jj.rs:69`). The documented `lightweight`/`annotated`
values change nothing.

**Ruling: remove, do not implement.** The settled design (v0 ticket 06) makes
the tag kind a property of the *VCS* — git annotated, jj lightweight — not a
user choice; `TagKind::Auto`'s own doc comment ("Defer to the VCS: annotated on
git, lightweight on jj") encodes exactly that. `annotated` is impossible under
jj (`adapters/vcs/jj.rs:3`), so the key advertises a third of its domain that can
never work, and implementing the overrides would require inventing semantics the
spec never settled. Removing an inert key changes no behaviour.

**Corrections:** The ticket is accurate. It omits the doc/spec cleanup that the
removal implies: the v0 spec §12 lists `tag_kind`, and
`docs/configuration.md:8-18` (the init block) and `:31` (the keys table) both
document it. Those must change too.

**Scope:** Standalone; one of the two dead-key rulings in this batch (see 22).

**Implementation sketch (removal):**

- `src/domain/config.rs`: delete the `TagKind` enum, its `FromStr`/`Display`,
  `Config.tag_kind` and its default, the overlay arm, the `table["tag_kind"]`
  line, `ConfigOverrides.tag_kind`, the `MOMO_TAG_KIND` arm, the `from_str`
  field, and the `defaults_match_the_spec` assertion. Grep confirms no other
  user.
- `docs/configuration.md`: drop the `tag_kind` table row, the `tag_kind = "auto"`
  line in the init block, and `MOMO_TAG_KIND` from the env list.
- `.scratch/momo-v0/spec.md` §12: drop `tag_kind`.
- `tests/*`: drop `MOMO_TAG_KIND` from the `env_remove` lists (cli, install,
  recording, manifest, monorepo, ci_output, release) and note this in 43.
- No adapter change: git stays annotated, jj stays lightweight.

## Resolution

Removed `TagKind` (enum, `FromStr`/`Display`), `Config.tag_kind`,
`ConfigOverrides.tag_kind`, the `MOMO_TAG_KIND` arm, the render line, and the
`defaults_match_the_spec` assertion. Dropped `tag_kind` from the init block, keys
table and env list in `docs/configuration.md`, from spec §12, from every test
`env_remove` list, and `tag_kind = "auto"` from this repo's own `momo.toml` (so
the strict key check from 23 still parses it). No adapter change: git stays
annotated, jj stays lightweight.
