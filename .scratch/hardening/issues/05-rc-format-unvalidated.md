# 05: `rc_format` is unvalidated, and the candidate counter only understands a trailing `{n}`

Type: bug
Status: ready-for-agent
Severity: medium
Area: domain/version
Verified: yes — read at the cited lines

## Symptom

Two related defects in the RC numbering:

1. `rc_format` is accepted with no requirement that it contain `{n}`. A template
   without it makes `candidate_version` emit the *same* version for every `n`.
2. `next_candidate` assumes the counter is the entire remainder after a fixed
   prefix. Any template with text after `{n}` never parses, so the number never
   advances.

Either way `momo candidate` mints a duplicate version, then `momo tag` fails with
`TagAlreadyExists`.

## Evidence

`src/domain/version.rs:91`:

```rust
let suffix = format.replace("{n}", &n.to_string());   // no check that {n} is present
```

`src/domain/version.rs:113-128`:

```rust
let Some((identifier, _)) = format.trim_start_matches('-').split_once("{n}") else {
    return 1;
};
recorded
    .into_iter()
    .filter(|version| same_release(version, target))
    .filter_map(|version| {
        version.pre.as_str().strip_prefix(identifier)?.parse::<u64>().ok()  // whole remainder as u64
    })
```

`rc_format` is read as a bare string (`src/domain/config.rs:339`, `:317`) with no
validation.

## Trigger

- `rc_format = "-rc"` → every candidate is `0.1.0-rc`; `next_candidate` returns
  `1` forever.
- `rc_format = "-{n}.rc"` → candidates are `0.1.0-1.rc`, `0.1.0-2.rc`, … but
  `next_candidate` parses `"1.rc"` as `u64`, fails, and returns `1` every time.

## Suggested direction

Reject an `rc_format` that does not contain exactly one `{n}` at config-parse
time, and parse the recorded pre-release against the template around `{n}`
(capturing digits between the fixed prefix and suffix) rather than assuming a
trailing number.

## Open questions for triage

- Is a custom `rc_format` with a suffix after `{n}` worth supporting, or should the
  template be constrained to `{n}` at the end and validated accordingly?

## Triage

**Verdict:** `ready-for-agent`. Both defects are confirmed and the fix is
well-scoped; the one judgement call — may a template carry text after `{n}`? —
is resolvable: require exactly one `{n}` and parse the recorded pre-release as
the digits *between* the fixed prefix and suffix, so any single-marker template
works without a surprising restriction.

**Corrections:** the evidence is accurate. Add that `candidate_version` replaces
*every* `{n}` while `next_candidate` splits on the *first*, so a template with
two markers (`-rc.{n}.{n}`) diverges as well — the "exactly one" validation
covers that. `rc_format` is read from three places (`momo.toml` via
`parse_string`, `MOMO_RC_FORMAT`, and `rc_format()` in `release.rs`); validate
once in the domain, not per call site.

**Scope:** no split. Independent of 04 / 06 / 07 / 36.

**Implementation sketch:**
- `src/domain/config.rs`: add `ConfigError::InvalidRcFormat { format }`. Add
  `Config::validate(&self) -> Result<(), ConfigError>` requiring `rc_format`,
  when set, to contain exactly one `{n}` and to yield a valid `Prerelease` for
  `n = 1` (reuse `candidate_version`). Call it from `cli::config::load` so both
  the file and the `MOMO_RC_FORMAT` layer are checked and the error names the
  bad value.
- `src/domain/version.rs`: in `next_candidate`, split the trimmed template into
  `(prefix, suffix)` at the single `{n}`; keep a recorded pre-release only when
  stripping `prefix` then `suffix` leaves a `u64`.
- Tests: config rejects `-rc` and `-rc.{n}.{n}`; `next_candidate` increments for
  `-{n}.rc` (`0.1.0-1.rc`, `0.1.0-2.rc` → `3`) and for the default; the existing
  different-template test still passes.
