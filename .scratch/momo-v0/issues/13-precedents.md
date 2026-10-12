# Precedents: existing versioning/changelog tools

Type: research
Status: resolved

## Question

Survey how established tools model this problem, so momo borrows proven choices:

`changesets` (JS), `towncrier` (Python fragments), `release-plz`,
`cargo-release`, `svu`, `semantic-release`, `git-cliff`, `scriv`.

For each, capture:

- how changes are **recorded**;
- **bump rules**, especially pre-1.0;
- **RC** handling;
- **changelog format**;
- **tag format**;
- **commit/push** behaviour;
- **dry-run / CI** output.

Call out explicitly who uses each of the two **recording models** (single file
vs fragments), since momo must support both.

## Answer

Full findings: [`../research/versioning-precedents.md`](../research/versioning-precedents.md).

Findings that bear directly on momo's open tickets:

- **Pre-1.0 is contested, and momo is in the minority-but-correct camp.** The
  literal-semver tools (changesets, cargo-release, semantic-release) jump `0.x` →
  `1.0.0` on a breaking change. The **zero-aware** camp keeps `0.x` and treats a
  breaking change as a **minor** bump: **release-plz** (default, citing Cargo's
  rules), **svu** (`--v0`), **git-cliff** (`breaking_always_bump_major=false`).
  momo's rule (`break` and `add` both bump MINOR at MAJOR 0) matches release-plz.
  → ticket 01 should cite this precedent.
- **The dotted RC form is the norm:** cargo-release `-rc.1`→`-rc.2`, svu `rc.1`,
  semantic-release `<pre>.1` — supporting ticket 05's lean toward `1.2.0-rc.1`
  over `-rc-1`. cargo-release's `release` level (strip the suffix to finalise) is
  a clean model for `release` after `rc`.
- **RC consume-vs-share:** **changesets** is the richest precedent — a `pre.json`
  state file, a per-package pre-version counter, fragments **retained** during pre
  mode and replayed into the stable release, then `pre exit` strips the suffix
  ("share, then strip"). → feeds ticket 05.
- **Fragment mechanics worth copying** (towncrier, scriv): a per-change file whose
  *filename* encodes identity, fragments deleted on collection, an
  orphan/uncategorised escape hatch, a configurable fragments directory — all
  merge-conflict-free by construction. → feeds tickets 02/03.
- **Single-file mechanics worth copying:** cargo-release's
  `pre-release-replacements` is a precise recipe for editing `Unreleased` in place
  (rename → stamp date → re-insert a fresh `Unreleased`). → feeds tickets 02/03.
- **Tag format:** `v`-prefix is near-universal. Annotated vs lightweight splits
  (cargo-release annotated by default; semantic-release lightweight). → ticket 06.
- **"Leave the tree dirty" is the majority default** (changesets, towncrier,
  scriv, release-plz, semantic-release); cargo-release is the outlier that commits
  *and pushes*. → ticket 06.
- **Dry-run conventions:** two recurring shapes — a read-only preview and a
  structured machine output (JSON). → ticket 07 should offer both.
- **Changelog format:** "Keep a Changelog" is the de-facto default among tools
  that own a changelog. → ticket 03 should treat it as the baseline vocabulary.

**Three recording models exist**; momo is models **A (fragments)** and **B
(single file)**, never **C (commit-message parsing)**.
