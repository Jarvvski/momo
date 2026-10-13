# Task-environment selection UX

Type: grilling
Status: resolved

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

## Answer

Settled (with one amendment to Q2):

1. **Selection: auto-detect with an explicit override.** `momo install` inspects
   the repo (`mise.toml` / `.mise/`) and installs what it finds; `momo install
   mise` names one explicitly. **No TUI in v0** — a single implementation means
   nothing to multi-select; the TUI arrives with gradle/bun.
2. **No environment found → graceful no-op with a warning, not an error.** momo
   is fully usable standalone (just the CLI), so `install` with nothing to install
   warns and suggests, rather than failing. **Several → install `mise`** (the only
   v0 impl) and note the others are unsupported; genuine multi-select arrives with
   the second implementation.
3. **The choice is stored in config** — `task_env` in `momo.toml` (ticket 04), so
   `install --update` / `uninstall` know what to refresh. A single value in v0;
   a list once multiple environments are supported.

Unblocks ticket 09 (mise integration).
