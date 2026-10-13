# momo config file & keys

Type: grilling
Status: resolved
Blocked by: 02, 06, 16

## Question

Does momo have a config file, and what lives in it?

- **(a)** `momo.toml` at the repo root; **(b)** `.momo.toml`; **(c)** config in
  the changelog front-matter; **(d)** no file — flags/env only.

Keys to settle (informed by other tickets):

- recording model (02), changelog path, fragment directory
- tag prefix (`v`?) and tag format (06)
- default task environment(s) (08)
- RC suffix format (05)

Sub-questions:

- Does `momo init` write it?
- Precedence between config file, environment variables, and CLI flags.
- Do we need a config file at all for v0, or can flags cover it?

## Answer

Settled:

1. **`momo.toml` at the repo root** (TOML) — option (a). Not `.momo.toml`, not
   changelog front-matter, not flags-only.
2. **Precedence:** CLI flags > `MOMO_*` environment variables > `momo.toml` >
   built-in defaults.
3. **Optional, with lazy creation** — write commands create the file (or the keys
   they need) on first use; read-only commands never create it.
4. **`momo init` writes only `momo.toml`** — it touches no other file (notably not
   `mise.toml` or any manifest).
5. **Keys (v0):** `recording` (02), the `changelog` path, `changes_dir`,
   `tag_prefix` and `tag_kind` (06), `task_env` (08), per-package overrides (16),
   and an optional `rc_format` (05).
