# momo config file & keys

Type: grilling
Status: open
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
