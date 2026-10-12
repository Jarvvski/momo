# Release-candidate semantics

Type: grilling
Status: open
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
