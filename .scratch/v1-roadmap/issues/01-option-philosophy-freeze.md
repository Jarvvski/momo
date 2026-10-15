# 01: Write the option philosophy and freeze the config surface

Type: task
Status: resolved
Points: 17
Bucket: Opinionated posture
Severity: medium
Area: docs + domain/config

## Problem

momo has nine config keys accumulated by history and no stated rule for what earns
a key. `AGENTS.md` carries the "short but sweet" constraint but no user sees it;
`docs/configuration.md` presents every key as equally legitimate. The
feature-request template actively invites the wrong thing: *"the command or
`momo.toml` key you would reach for"*
(`.github/ISSUE_TEMPLATE/feature_request.yml:16`). Every key is a permanent
compat promise, a doc row, an env var and a test path.

## Evidence

- Nine keys, one count assertion: `src/domain/config/keys.rs:14`; three package
  keys at `:27`; `src/cli/commands.rs:37` (`VERBS`, `len == 9`).
- The env surface is **eight** vars (`src/domain/config/env.rs:33-50`) but
  `tests/ci_output.rs:19` clears only seven — `MOMO_COMMAND_TIMEOUT` already
  drifted. Proof the surface is unpinned.
- `rc_format` costs ~100 lines in `src/domain/version.rs` (`rc_parts`,
  `candidate_prerelease`, `validate_rc_format`, `next_candidate`, `RcFormatError`)
  to rename one concept `GLOSSARY.md:19` fixes, and needs a validation rule
  explaining its own footgun (`docs/configuration.md:44`).
- Precedent for shrinking pre-1.0: `CHANGELOG.md` records removing `momo notes`.

## Proposal

**A freeze that is only prose is not a freeze.** Ship the doc *and* the tests that
pin the surface — the repo already proves the pattern works (`VERBS.len() == 9`).

**Key triage** (kept / scoped / managed / deprecated):

| Key | Verdict | Why |
|---|---|---|
| `recording` | kept | A real workflow split; prose demotes `fragments` (see ticket 11). |
| `changelog` | kept | A fact about where the file already is, not a bikeshed. |
| `changes_dir` | **scoped** | Meaningful only when `recording = "fragments"`. |
| `tag_prefix` | kept | The one key migration genuinely requires (`docs/migrating.md:69`). |
| `remote` | kept | Escape hatch for a VCS-less repo or an underivable remote. |
| `task_env` | **managed** | Written by `momo install`; move to a "managed" heading. |
| `rc_format` | **deprecated** | A templating mini-language for a fixed concept. |
| `command_timeout` | kept | A safety valve, not taste. **Not** frozen — see risks. |
| `packages.*` | kept | Escape hatches for monorepo reality. |

**Deprecate `rc_format`, do not yank.** Keep parsing and honouring it, stop
documenting it, remove at 1.0. Do **not** emit a runtime warning — it would
pollute CI stderr for someone who legitimately set it. momo ships into consumer
repos (ADR 0001), so a config that starts erroring is a real break.

**Operationally, "frozen" means** the recognised keys are a budget pinned **by
name**, not length, so a swap cannot sneak through. Adding a key means editing the
constant, the name test, `docs/configuration.md`, and the philosophy doc in one
deliberate PR.

## Surface

- New `docs/option-philosophy.md`: (1) a key exists only where reality varies or a
  safety valve is needed, never for taste; (2) new keys are **not accepted by
  default** — a request must show the variation is real, not expressible by an
  existing key, and that a default would be wrong for a substantial minority;
  (3) the surface is frozen and pinned by tests; (4) escape hatches are quiet
  (env var, `--flag`, `--rev`, per-package override); (5) pre-1.0 the surface may
  shrink.
- `docs/configuration.md`: a "Not accepting new keys" pointer at the top of Keys;
  move `rc_format` to a Deprecated note with the removal version; move `task_env`
  under Managed.
- `docs/contributing.md`: a "Configuration policy" subsection linking the doc.
- `.github/ISSUE_TEMPLATE/feature_request.yml`: "Describe the behaviour, **not the
  key**"; note that key requests are closed by default.
- `.github/PULL_REQUEST_TEMPLATE.md`: a checklist line — "adds a config key or
  verb? it updates the frozen-surface test and the option philosophy."

## Implementation sketch

- `src/domain/config/keys.rs`: make `KNOWN_KEYS` visible to tests; add to
  `src/domain/config/tests.rs`:
  ```rust
  #[test]
  fn the_config_surface_is_frozen() {
      assert_eq!(KNOWN_KEYS, ["changelog", "changes_dir", "command_timeout",
          "packages", "rc_format", "recording", "remote", "tag_prefix", "task_env"]);
      assert_eq!(KNOWN_PACKAGE_KEYS, ["identity", "manifest", "slug"]);
  }
  ```
- `src/domain/config/env.rs`: a unit test pinning the eight `MOMO_*` names; fix the
  seven-item list in `tests/ci_output.rs:19`.
- `src/domain/config.rs`: extend the round-trip test to assert the exact key set
  `init` renders.
- `src/cli/commands.rs`: extend `every_command_has_an_installed_verb` (line 507)
  to pin verb **names**, not just the count.
- Docs and templates above. No CLI change.

## Scoring

Points **17** = Value 4×2 + Ease 5 + Novelty 4. Novel (nobody freezes a versioning
tool's config), the easiest possible (docs + tests), high value (it protects the
whole surface and makes every future "add a flag?" answerable by link).

## Risks

- **Alienating a user with a real need.** The policy requires *showing* the
  variation is real; escape hatches stay documented.
- **The freeze test becoming bureaucratic.** It is one list; bumping it is one
  line and is the point.
- **Over-freezing `command_timeout`.** It stays: it is a safety valve, not dogma,
  and the philosophy doc says so.

## Related

- 09 (blessed path), 11 (state the opinions), 10 (`INSTALLABLE_VERBS` split).
- `.github/ISSUE_TEMPLATE/feature_request.yml` invites the wrong thing today.
