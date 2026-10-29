# 06: A PEP 508 requirement with whitespace before `[extras]` is misparsed

Type: bug
Status: ready-for-agent
Severity: medium
Area: adapters/manifest/python/requirement
Verified: yes — code path confirmed against the PEP 508 grammar

## Symptom

`acme [extra]>=1.0` — valid PEP 508, where whitespace is allowed between the name
and the extras — is parsed with `[extra]>=1.0` as the *specifier*. The specifier
then fails to parse, so the requirement is reported unchecked (`satisfied`
returns `None`) and never widened, instead of being read as name `acme`, extras
`extra`, specifier `>=1.0`.

## Evidence

`src/adapters/manifest/python/requirement.rs:59-61` only recognises `[` as the
start of extras when it immediately follows the name (`bytes.get(index) == Some(&b'[')`).
With a space, the scan stops at the space and the extras are folded into the
specifier. The PEP 508 grammar is `name wsp* extras? wsp* versionspec? wsp* …`.
The existing test (`requirement.rs:379`) only covers the no-space form
`acme[extra]>=1.2,<2`.

Impact fails safe (the edge is reported unchecked rather than mis-evaluated), so
this is a false "cannot check" rather than a wrong answer.

## Fix direction

After scanning the name, skip whitespace before checking for `[` (and keep the
skipped whitespace in the requirement's `prefix`, as the current code does for
the specifier gap). Add tests for `acme [extra]>=1.0` and
`acme [a, b] >=1.0` asserting the parsed `name`, `specifier` and `prefix`.
