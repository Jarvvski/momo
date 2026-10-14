# Opinions

momo is opinionated. This page is the single place that states what it believes and which parts are negotiable, so a reader can answer "what does momo assume, and what can I change?" without piecing it together from the README, the ADRs and the source.

The spine: **assert the outcomes that cause bikeshedding; default where reality varies.** The semver rules, the changelog shape and the release/tag split *are* the product — they are asserted, tested, and not configurable. Where a project's reality genuinely differs — where its changelog lives, what its tags look like, how changes are recorded — momo defaults and lets you override.

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

## Asserted

These are the decisions a versioning tool exists to make. They are pinned by tests, not by configuration, because a configurable bump policy is a configurable semver — which is no policy at all.

**Three Kinds, and the highest wins.** Every Change is exactly one of `break`, `add` or `fix`. There is no `changed`, `deprecated` or `security` Kind; those are Keep a Changelog headings momo preserves but does not read. When a release mixes Kinds the highest wins (`break > add > fix`), so one breaking change in a release of twenty fixes makes the whole release breaking.

**The zero-aware bump.** At MAJOR 0 a `break` and an `add` both bump MINOR, and a `fix` bumps PATCH; at MAJOR ≥ 1 a `break` bumps MAJOR, `add` MINOR and `fix` PATCH. The point of pre-1.0 is that the minor is where breakage is expected, so below 1.0 `break` and `add` collapse to the same bump. The version alone therefore cannot signal a pre-1.0 break — the `Breaking` changelog section is that signal. See [ADR 0005](adr/0005-zero-aware-bump-policy.md).

**Baseline `0.0.0`; absence is not `0.0.0`.** A package that has never released has *no* version, and momo reports it that way (`"current": null`) rather than pretending it sits at `0.0.0`. `0.0.0` is a legal release, so a package that genuinely shipped `0.0.0` is not the same as one that never released. The first release still bumps from `0.0.0` under the rules above.

**The changelog shape.** Releases are `## [x.y.z] - YYYY-MM-DD`, newest first, grouped under `### Breaking`, `### Added` and `### Fixed` in Keep a Changelog style. The document is preserved: momo edits only the spans it owns — the Unreleased body, its own footer links, and the release headings it inserts — and passes everything else through byte-for-byte. See [ADR 0003](adr/0003-preserve-the-changelog-document.md).

**The tag shape.** A tag is `<prefix><version>` (`v1.2.3` by default); in a monorepo it is `<slug>-<prefix><version>` (`cargo-momo-v1.2.3`). momo reads tag *names*, not their targets, so a tag's commit only matters for the compare links in the footer.

**Release → commit → tag.** `release` mints the version and writes the changelog and the ledger; it never commits and never tags. You commit the result, then `momo tag` tags it. The split is deliberate: it is what lets the tag land on the commit that carries the changelog.

**UTC dates, immutable tags, no CI sniffing.** A release heading is dated in UTC so a release is reproducible across machines and runners. An existing tag is never moved, rewritten or deleted. And momo never reads `CI_*`/`GITHUB_*` — it reports, and CI decides what to do about the report, so it behaves the same on a laptop and on a runner.

## Defaulted

These are where projects genuinely differ. Each is a default with a key to override it; none of them changes the semver rules.

**The recording model defaults to `changelog`.** The changelog is *the* model: changes accumulate under `## Unreleased` and that section is the source of unreleased changes. `recording = "fragments"` is a documented escape hatch for changes that arrive asynchronously — CI-authored, monorepo, many concurrent contributors — where a single file would be a merge-conflict magnet. It is not a second way of thinking about releases; see [Usage](usage.md#recording-models).

**Keys exist where reality varies.** The `changelog` path, `tag_prefix`, `remote` and `command_timeout` all default, and per-package overrides (`identity`, `slug`, `manifest`) cover monorepo reality. See [Configuration](configuration.md).

## The deviations, owned out loud

Two of the asserted rules part company with a well-known convention. Both are deliberate, and both are stated here rather than left for a reader to trip over.

### Cargo semver at `0.0.x`

momo's rules match Cargo's compatibility boundaries at `0.x` and at `≥ 1`. The one divergence is `0.0.x`: Cargo treats every `0.0.z` as breaking, so a fix at `0.0.0` would bump `0.1.0`; momo bumps PATCH, `0.0.0` → `0.0.1`. This is deliberate — `0.0.x` is bootstrap scaffolding, not a compatibility promise, and a fix is a fix. The divergence is bounded to `0.0.x`: the moment a project leaves it, the policy is Cargo's. Recorded in [ADR 0005](adr/0005-zero-aware-bump-policy.md).

### Keep a Changelog's `Changed`

Keep a Changelog's canonical headings include `Changed`; momo's breaking-change section is `Breaking`. momo interprets exactly three headings — `Breaking`, `Added`, `Fixed` — and preserves every other heading without reading it, so a `### Changed` section survives but does not affect the version. The rename is intentional: `Breaking` names what the section means (a break, which drives the bump) where `Changed` only says that something moved. See [Migrating](migrating.md#the-bump-only-reads-three-headings).
