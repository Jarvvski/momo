# Release-candidate semantics

Type: grilling
Status: resolved
Blocked by: 01

## Question

Define release candidates precisely. The sketch: append `rc-<increment>` to the
version. Settle:

- **Exact format:** `1.2.0-rc-1` vs `1.2.0-rc.1` (note: `-rc.1` is the
  SemVer-conventional dotted form).
- **Increment scope:** per target version, or global?
- **How an RC is minted** from the Unreleased Changes, and whether it
  **consumes** them (so the final Release starts empty) or **shares** them.
- **RC → final:** how the eventual release relates to the last RC.
- Do RCs appear in the changelog, and are they tagged (see 06)?
- Interaction with the kinds: does an RC bump MAJOR/MINOR/PATCH from the
  Unreleased Kinds first, then add the suffix?

## Answer

Settled (all recommendations accepted):

1. **Format:** dotted `1.2.0-rc.1` (the SemVer convention; matches cargo-release
   `-rc.1` and svu `rc.1`).
2. **Increment scope:** per target version — `1.2.0-rc.1` → `1.2.0-rc.2`; if the
   target changes (a new kind moves the bump), the counter **restarts at `1`**.
3. **Share, not consume:** fragments survive an RC and are consumed by the final
   release. `momo rc` writes an RC changelog heading but does **not** delete
   fragments.
4. **Computation:** `momo rc` computes the bump from the unreleased kinds exactly
   as `release` would, then appends `-rc.N`. It performs the same file writes as
   `release` (changelog + manifests) but mints a pre-release and leaves fragments
   in place. The final `release` reuses those same changes and drops the suffix.

Tagging follows ticket 06: neither `release` nor `rc` tags; `momo tag` does, after
you commit.
