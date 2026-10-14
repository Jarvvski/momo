# Relicense to MIT OR Apache-2.0

momo shipped as `GPL-3.0-or-later`. For a developer tool that is the single biggest adoption limiter: many organisations ban GPL from their toolchain outright, and the Rust norm for a CLI is the permissive dual `MIT OR Apache-2.0`. Copyleft was not a considered stance here, so it read as an accident rather than a choice. The copyright holder is the sole human contributor over the whole history — every commit is authored by `Adam Jarvis <adam@ameba.ai>` or a bot — so the relicense is the holder's own grant and needs no third-party consent. momo is therefore dual-licensed **`MIT OR Apache-2.0`**, the recipient's choice, which matches the rest of the CLI ecosystem and keeps the Apache-2.0 patent grant available.

## Considered Options

- **Keep `GPL-3.0-or-later`** — rejected: blocks a large slice of the intended audience for a tool whose value grows with adoption.
- **Weak copyleft (`MPL-2.0`)** — rejected: a middle ground nobody asked for, and still unfamiliar enough to trip licence scanners.
- **Dual `MIT OR Apache-2.0`** — chosen: the Rust CLI default; permissive, and the Apache arm adds an explicit patent grant.

## Consequences

- `Cargo.toml` declares `license = "MIT OR Apache-2.0"`; `LICENSE-MIT` and `LICENSE-APACHE` replace the single GPL `LICENSE`.
- The README badge and `## License` section state the dual licence, and `deny.toml`'s dependency `licenses.allow` no longer lists `GPL-3.0-or-later`.
- Contributions are accepted under the same dual terms (the crate uses no CLA).
