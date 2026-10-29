# 05: PEP 440 rendering refuses uppercase pre-release kinds and skips separator normalisation

Type: bug
Status: ready-for-agent
Severity: medium
Area: adapters/manifest/python/pep440
Verified: yes — code path confirmed against the PEP 440 grammar

## Symptom

Two gaps in rendering a semver version as PEP 440:

1. `classify` matches pre-release kind aliases case-sensitively, so a valid
   uppercase spelling (`RC`, `Rc`, `ALPHA`, `BETA`, `PRE`) is reported
   `Unrepresentable` and aborts the release, even though PEP 440 is
   case-insensitive and `packaging` normalises it.
2. Build metadata is lowercased but its `-`/`_` separators are not normalised to
   `.`, so `1.2.3+build-1` is written where PEP 440's canonical form is
   `1.2.3+build.1`.

## Evidence

`src/adapters/manifest/python/pep440.rs:70-90` (`classify`) compares with `==`
and `strip_prefix`, so `classify("RC")` is `None`; the parser's own
`take_pre` (`:272`) uses `strip_ci` and *is* case-insensitive — the two disagree.
`render` (`:25-28`) only calls `to_ascii_lowercase()` on the build string. A tag
`v1.2.0-RC.1` (a valid semver pre-release, and reachable through a user-configured
`rc_format` such as `-RC.{n}`) renders to `Err(Unrepresentable)`, which
`apply_version` (`mod.rs:280`) turns into a fatal conflict.

## Fix direction

Make `classify` match aliases case-insensitively (fold `head` to ASCII
lowercase, or use `eq_ignore_ascii_case`/`strip_ci` as `take_pre` does), and
normalise `-` and `_` to `.` in the rendered build metadata. Add tests for an
uppercase kind and for a build string with a hyphen and an underscore.
