# The option philosophy

momo's configuration surface is deliberately small and deliberately closed. This page is the rule that keeps it that way, and the answer to most requests to "just add one more key".

## 1. A key exists only where reality varies, or a safety valve is needed

A `momo.toml` key earns its place by one of two tests:

- **Reality varies.** Projects genuinely differ in a way momo cannot infer: the tag prefix they already use, the name of their changelog, whether they record Changes as changelog entries or fragments. momo cannot guess, so the project must be able to say.
- **A safety valve is needed.** A default is right for almost everyone, but there is a documented failure a project must be able to escape: a very large repository where a VCS read legitimately outlives the default `command_timeout`.

Everything else is **taste** — a preference where the default is already fine and the variation is cosmetic. Taste does not earn a key. A key is a permanent compatibility promise: it is parsed forever, documented, given an environment variable, and pinned by a test. It has to buy more than a preference.

## 2. New keys are not accepted by default

A request for a key has to show all three of these:

1. The variation is **real**, not a preference.
2. It is **not already expressible** by an existing key.
3. A single default would be **wrong for a substantial minority** of projects — not merely inconvenient for one.

An escape hatch that is not a key is always preferred: an environment variable, a `--flag`, `--rev`, or a per-package override. Those carry no compatibility promise and no doc row. If a request cannot pass all three tests, the answer is to describe the behaviour and let the surface stay as it is.

## 3. The surface is frozen and pinned by tests

The recognised keys are a budget pinned **by name**, not by length, so a swap cannot sneak through. `KNOWN_KEYS` and `KNOWN_PACKAGE_KEYS` live in `src/domain/config/keys.rs`; the eight `MOMO_*` names live in `src/domain/config/env.rs`; and the tests pin the exact key set `momo init` renders and the verb names in `src/cli/commands.rs`.

Adding a key therefore means editing the constant, the name test, `docs/configuration.md` and this page in one deliberate pull request. The test failing is the point: it turns a quiet addition into a decision someone has to make on purpose.

## 4. Escape hatches are quiet

The escape hatches — an environment variable, a `--flag`, `--rev`, a per-package override — are silent. An unused hatch changes nothing, and a used one does not lecture.

Deprecation is quiet too. A key scheduled for removal is still parsed and honoured without a runtime warning. momo ships into consumer repos ([ADR 0001](adr/0001-distribute-mise-tasks-as-files.md)), so a config that starts erroring is a real break, and polluting CI stderr for someone who legitimately set a key is a cost with no benefit. `rc_format` is deprecated this way: still honoured, no longer documented as a supported knob, and scheduled for removal at 1.0.

## 5. Pre-1.0 the surface may shrink

Before 1.0 the surface may **shrink**, not only grow. A key that stops earning its place is removed deliberately, with the change recorded in the changelog, rather than carried forever. `momo notes` and the `tag_kind` key were removed this way, and `rc_format` is on the same path. Shrinking is the release valve that keeps the surface honest.
