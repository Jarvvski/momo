# 04: Poetry caret `^0` and `^0.0` compute the wrong upper bound

Type: bug
Status: resolved
Severity: high
Area: adapters/manifest/python
Verified: yes — read at the cited lines; compared with poetry-core's `next_breaking`

## Symptom

A dependent declaring `acme = "^0"` or `acme = "^0.0"` is reported unsatisfied at any `0.x` target and spuriously rewritten.

## Evidence

`caret_upper` (`src/adapters/manifest/python/requirement.rs:335-346`) cannot tell an omitted segment from an explicit `0`, so both `^0` (`release [0]`) and `^0.0` (`release [0,0]`) take the `[0, 0, patch + 1]` branch and yield `<0.0.1`. poetry-core's `next_breaking` gives `^0 → <1.0.0` and `^0.0 → <0.1.0`:

```python
if self.major > 0 or self.minor is None: return next_major()
if self.minor > 0 or self.patch is None: return next_minor()
return next_patch()
```

## Trigger

`satisfied("^0", "0.9.9")` → `Some(false)` (Poetry: `true`); `satisfied("^0.0", "0.0.9")` → `Some(false)` (Poetry: `true`).

## Suggested direction

Mirror poetry-core's `next_breaking` exactly, which keys on *whether a segment is present*, not on the count alone:

- break at the next major when `major > 0` **or** the minor is absent (`^0`, `^1.2.3`);
- else break at the next minor when `minor > 0` **or** the patch is absent (`^0.2.3`, `^0.0`);
- else break at the next patch (`^0.0.0`).

**A first draft of this ticket said "length 1 → next major, length 2 → next minor, length 3 → next patch", which is wrong**: it would send `^0.2.3` (length 3) to the next *patch* instead of the next *minor*, regressing an existing correct case. The implementation must key on presence, not length. Add tests for `^0` and `^0.0`, and keep the existing `^0.2.3`/`^0.0.3` cases green.
