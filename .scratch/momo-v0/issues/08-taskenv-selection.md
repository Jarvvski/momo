# Task-environment selection UX

Type: grilling
Status: claimed

## Question

How does momo choose which task environment to install into?

- **(a)** explicit argument only (`momo install mise`);
- **(b)** auto-detect (look for `mise.toml`, `package.json`, `build.gradle`, …);
- **(c)** a TUI multi-select list (the sketch mentions one);
- **(d)** detect, then confirm.

Settle:

- Behaviour when **several** environments are present.
- Whether the selection is **stored in config** (see 04) so later commands know
  the environment.
- What `momo install` with **no argument** does.
