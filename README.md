<p align="center">
  <strong>momo</strong>
</p>

<p align="center">
  <a href="LICENSE"><img alt="License: GPL-3.0-or-later" src="https://img.shields.io/badge/license-GPL--3.0--or--later-blue"></a>
</p>

---

> **Description pending.** momo is a new open-source project; its scope is still
> being defined.

## Build & develop

Tooling is driven by [mise](https://mise.jdx.dev):

```bash
mise install            # pin the Rust toolchain
mise run run            # cargo run --release -p momo
mise run build          # cargo build --release --workspace
mise run test           # cargo test --workspace
mise run fmt            # cargo fmt --all
mise run lint           # cargo clippy --all-targets -- -D warnings
mise run check          # fmt + lint + build + test (the pre-land gate)
mise run install        # install the binary to ~/.cargo/bin
```

Plain cargo works too:

```bash
cargo run -p momo
```

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).
