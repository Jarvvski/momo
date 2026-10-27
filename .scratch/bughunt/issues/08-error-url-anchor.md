# 08: Every error's docs URL points at a GitHub anchor that does not exist

Type: bug
Status: resolved
Severity: medium
Area: cli/diagnostic, docs
Verified: yes — reproduced against the live GitHub rendering

## Symptom

The `url` field momo attaches to an error (and prints under `--format json`) 404s — the fragment names a heading id GitHub never generates.

## Evidence

`diagnose` builds the fallback URL with `code.replace('.', "-")` (`src/cli/diagnostic.rs:75`), turning `momo.change.empty-text` into `#momo-change-empty-text`. GitHub's anchor slugger *removes* dots rather than hyphenating them. Fetched from the live page, the actual ids are `momochangeempty-text`, `momoconfigempty-path`, `momomanifestinherited-version`, … — no hyphens where the code puts them.

The wrong form is also baked into `docs/errors.md:35` and `docs/ci.md:192`, and pinned by the test at `src/cli/diagnostic.rs:255`.

## Trigger

Any failing command under `--format json`; read the `url`.

## Suggested direction

Strip the dots instead of hyphenating: `code.replace('.', "")` (matching GitHub's slug for a heading of the bare code). Update `docs/errors.md`, `docs/ci.md` and the test to the resolvable fragment. Confirm against the rendered page before landing.
