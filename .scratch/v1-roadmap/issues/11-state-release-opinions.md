# 11: State the release opinions

Type: task
Status: ready-for-human
Points: 13
Bucket: Opinionated posture
Severity: medium
Area: docs + domain/version

## Problem

momo's opinions are scattered across `README.md`, `docs/usage.md`,
`docs/migrating.md`, ADRs and code comments. Nobody can read one page and answer
"what does momo believe, and which parts are negotiable?" The contentious ones —
the zero-aware rule, the Keep-a-Changelog deviation, the tag scheme, the
release→commit→tag split, the recording default — are stated in fragments and never
owned as a set.

## Evidence

- `README.md:70-79` (zero-aware), `src/domain/change.rs:27-33` (`Kind::heading`
  emits `Breaking`, where KaC emits `Changed` — momo already deviates, unstated).
- `docs/usage.md:130` / `GLOSSARY.md:21` (the ledger's role), `docs/migrating.md`.
- The `0.0.x` divergence from Cargo semver: release-plz's docs explicitly warn the
  `add`→minor-at-0.x behaviour "violates the Cargo SemVer rules" — the exact rule
  momo bakes in.

## Proposal

**The spine: assert outcomes that cause bikeshedding; default where reality
varies.**

| Opinion | Verdict |
|---|---|
| Three Kinds; `break > add > fix`; highest wins | **Asserted** (tested) |
| Zero-aware bump: at MAJOR 0 `break`/`add` → MINOR, `fix` → PATCH | **Asserted** (tested) |
| Baseline `0.0.0`; absence ≠ `0.0.0` | **Asserted** |
| Changelog shape: `Breaking`/`Added`/`Fixed`, KaC-style, preserved document | **Asserted** |
| Tag shape `<prefix><version>`; per-package `<slug>-<prefix><version>` | **Asserted** |
| Release → commit → tag; `release` never commits or tags | **Asserted** |
| UTC dates; never overwrite a tag; never read `CI_*`/`GITHUB_*` | **Asserted** |
| Recording model: `changelog` default | **Defaulted** (key) |
| `changelog` path, `tag_prefix`, `remote`, `command_timeout`, package overrides | **Defaulted** (keys) |

**Own the Cargo deviation out loud.** The rules match Cargo's compatibility
boundaries at `0.x` and `≥1`; the one divergence is `0.0.x` (Cargo treats every
`0.0.z` as breaking, momo bumps PATCH for a `fix` at `0.0.0`). State it, justify it
("`0.0.x` is bootstrap scaffolding; a fix is a fix"), accept it as deliberate. Also
own that at `0.x`, `break` and `add` collapse to the same bump, so the `Breaking`
changelog section is the only pre-1.0 break signal.

**Own the KaC deviation:** `Breaking` where KaC says `Changed` — intentional.

**Demote `fragments` to a documented escape hatch** (in prose, not config): the
changelog is *the* model; fragments is for changes that arrive asynchronously
(CI-authored, monorepo, many concurrent contributors). This is Biome's shape:
opinionated default, configurable escape.

**`momo explain` — yes; `momo doctor` — no.** `explain` renders the *resolved*
opinions and config provenance (which layer won), which nothing else does.
`doctor`'s checks fold into `status` **warnings** (tags exist but none match the
prefix; `.momo/version` behind the newest tag; manifest version disagrees with the
last release). Note honestly: `explain` adds a verb, which fights the freeze — make
it a deliberate, recorded exception, or ship the doc alone first.

## Surface

- New `docs/opinions.md` (the asserted/defaulted table, in prose).
- New `docs/adr/0005-zero-aware-bump-policy.md` (the decision, the Cargo deviation,
  alternatives).
- `README.md`: a short "Opinions" section after the semver table, linking the doc.
- `docs/usage.md:13-22`: demote fragments in prose.
- Optional `momo explain` (read-only); `status` warnings.

## Implementation sketch

- Docs + ADR + README section — no code.
- Pinning tests: extend `src/domain/version.rs` tests to pin `Kind::bump` at MAJOR 0
  and ≥1; pin the three heading strings in `src/domain/change.rs`; pin
  `DEFAULT_RC_FORMAT`.
- `explain` (optional): `Command::Explain`; `load_layers(root) -> (ConfigOverrides,
  ConfigOverrides)` in `src/cli/config.rs` (provenance is a diff against
  `Config::default()`); an `Explain` report; bump `VERBS`/`INSTALLABLE_VERBS` to 10.
- `status` warnings: read `Vcs::latest_tag`/`latest_release_tag` and the ledger; no
  port change.

## Scoring

Points **13** = Value 3×2 + Ease 4 + Novelty 3. The most novel *positioning* item
(stating the deviations, especially the Cargo one, is rare); cheap; moderate value.

## Risks

- **`explain` becomes a second `status`.** It reports configuration and opinions,
  never changes/versions.
- **The ADR re-litigates a settled decision.** Record, don't debate.
- **Stating the Cargo deviation invites "fix it" issues.** The philosophy doc (01)
  says it is deliberate.
- **`status` warnings pollute CI stderr.** Warnings only, never non-zero exit.

## Related

- 01 (philosophy), 09 (blessed path), ADR 0003, `docs/migrating.md`.
