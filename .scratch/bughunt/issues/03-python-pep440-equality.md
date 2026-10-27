# 03: Python exact-match specifiers ignore PEP 440's insignificant trailing zeros

Type: bug
Status: resolved
Severity: high
Area: adapters/manifest/python
Verified: yes — read at the cited lines; compared with `packaging`'s ordering key

## Symptom

`==1.2` (and a bare `1.2`) is reported unsatisfied at `1.2.0`, which pip accepts. `!=1.2` is reported satisfied. A dependent whose range already matches is reported unsatisfied and spuriously rewritten by `--sync-ranges`.

## Evidence

`Specifier::matches` compares with the derived `PartialEq` (`src/adapters/manifest/python/requirement.rs:180-193`), but `Pep440Version` derives `PartialEq` structurally (`pep440.rs:99`) while its `Ord` trims trailing zeros (`pep440.rs:350-371`). So `parse("1.2")` (`release [1,2]`) and the target `1.2.0` (`release [1,2,0]`) compare `Equal` under `cmp` but `!=` under `==` — an Eq/Ord contract violation, and a divergence from PEP 440, which treats `1.2` and `1.2.0` as equal.

## Trigger

`satisfied("==1.2", "1.2.0")` → `Some(false)` (pip: `true`); `satisfied("!=1.2", "1.2.0")` → `Some(true)` (pip: `false`).

## Suggested direction

Compare with the ordering key, not the derived equality: `candidate == version` becomes `candidate.cmp(version) == Ordering::Equal` (and `!=` its negation). If the Eq/Ord mismatch should be closed at the type instead, implement `PartialEq`/`Hash` consistently with `Ord` by comparing the trimmed release. Add tests for `==1.2` vs `1.2.0` and the bare `1.2` form.
