# 09: Docs — README and the `--check` contract

**What to build:** A README that explains what momo is and how to run it
standalone and through mise, and that documents the `status --check` exit-code
contract (non-zero when there is nothing Unreleased). This is the documentation
debt recorded by the CI-output decision.

**Blocked by:** 06

**Spec:** `.scratch/momo-v0/spec.md` §13 (the documented exit-code contract)

**Status:** ready-for-agent

- [ ] README covers installation, the verb surface, the three Kinds, and the semver rules.
- [ ] The `status --check` exit-code contract is documented explicitly.
- [ ] The config file, its keys, and the precedence order are documented.
- [ ] Every command shown in the README has been run and works.
