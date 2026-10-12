# Rust crate & dependency landscape (ticket 14)

Type: research
Status: resolved

Scope: recommend momo's v0 dependency set with trade-offs, and flag anything
that fights the repo constraints — `unsafe_code = "forbid"`, edition 2024,
Rust 1.99 (`rust-version`), clippy warnings-as-errors, minimal deps.

Local toolchain confirmed for this research: `cargo 1.99.0`, `rustc 1.99.0`,
`jj 0.46.0`, `git 2.56.0`. (Read-only inspection; no VCS commands were run
against the repo.)

---

## TL;DR recommendation

| Concern | Pick | Version |
|---|---|---|
| CLI parsing | **`clap` derive** | 4.6 |
| TUI multi-select | **`inquire`** (trimmed features) | 0.9 |
| TOML round-trip | **`toml_edit`** (`DocumentMut`) | 0.25 |
| Semver | **`semver`** | 1.0 |
| Dates | **`time`** (UTC, `Date` Display) | 0.3 |
| Git access | **shell out to `git`** (no `git2`) | — |
| jj access | **shell out to `jj`** (`jj tag set/list`) | 0.46+ |
| Errors | **`anyhow`** top-level + **`thiserror`** typed | 1.0 / 2.0 |

Nothing in this set violates `unsafe_code = "forbid"`. **That lint is
per-crate**: `[lints]` in `Cargo.toml` configures lints for *this package*
only, and rustc lints never apply across a dependency boundary
(<https://doc.rust-lang.org/cargo/reference/manifest.html#the-lints-section>,
<https://doc.rust-lang.org/rustc/lints/levels.html>). So a dependency using
`unsafe` internally (e.g. `git2`/FFI, `crossterm`) does **not** break momo's
forbid — but it *is* a signal about build weight and auditability, which is the
real reason to avoid `git2` here.

Every crate below declares an MSRV at or below 1.99, so the `rust-version`
floor is not a constraint on any candidate (lowest relevant MSRVs: `time` and
`ratatui` at 1.88, `clap`/`toml_edit` at 1.85).

---

## 1. CLI parsing — `clap` (derive) vs `argh` vs `lexopt`

| | `clap` 4.6 | `argh` 0.1.19 | `lexopt` 0.3.2 |
|---|---|---|---|
| Model | derive + builder | derive only | imperative, no macros |
| Deps | `clap_builder` (+ `clap_derive` with `derive`) | `argh_derive`, `argh_shared` | **none** |
| Help/usage gen | full, styled, `--help`/`--version` | good, Fuchsia-spec | **none** (you write help) |
| Subcommands | first-class, nested | supported | hand-rolled |
| Downloads (recent 90d) | ~254M | ~2.9M | ~2.9M |
| Edition / MSRV | 2024 / 1.85 | 2018 / — | 2018 / 1.31 |

Sources: crates.io API for the three crates
(<https://crates.io/crates/clap>, <https://crates.io/crates/argh>,
<https://crates.io/crates/lexopt>); `argh` README
(<https://github.com/google/argh>); `lexopt` README states it is "one file, no
dependencies, no macros … there is no help generation"
(<https://github.com/blyxxyz/lexopt>); `clap` feature map from
<https://crates.io/api/v1/crates/clap/4.6.7>.

**Recommendation: `clap` with `features = ["derive"]`.** momo has 8 verbs plus
a global `--dry-run` and real `--help`/`--version` output (tickets 07/11) — the
help and subcommand ergonomics are the whole point of the CLI. `argh` is
smaller but is optimised for *code size* on embedded/Fuchsia-style tools and
gives a terser help style; `lexopt` is the true zero-dep option but makes you
hand-write help, usage and error messages, which is the opposite of "short but
sweet". `clap`'s default features are `std, color, help, usage,
error-context, suggestions`; if binary size matters you can drop `color`
(still keeps `help`/`usage`). Derive vs builder: derive is less code for a
static command tree like momo's; the builder API is only worth it if the tree
is built dynamically (it isn't).

## 2. TUI multi-select (task-environment picker, ticket 08) — `inquire` vs `dialoguer` vs `ratatui`

All three expose a multi-select; the differences are weight and how much you
build yourself.

- **`inquire` 0.9.4** — first-class `MultiSelect` prompt with filtering,
  pagination, defaults, custom formatters/validators. Its README advertises an
  **"unsafe forbidden"** badge (it forbids `unsafe` in its own crate). Always-on
  deps: `bitflags`, `dyn-clone`, `unicode-segmentation`, `unicode-width`; the
  default backend is `crossterm`. Default features are `macros, crossterm,
  one-liners, fuzzy` — trimmable.
  Sources: <https://crates.io/crates/inquire>,
  <https://github.com/mikaelmello/inquire>.
- **`dialoguer` 0.12.0** — `MultiSelect` too, but a plainer UX. Always-on deps
  `console` + `shell-words`; default features `editor` (`tempfile`) and
  `password` (`zeroize`) are irrelevant to us and should be disabled.
  Sources: <https://crates.io/crates/dialoguer>,
  <https://github.com/console-rs/dialoguer>.
- **`ratatui` 0.30.2** — a *framework* for full-screen TUIs, not a prompt. You
  would hand-build the list widget, selection state and event loop, and it pulls
  `ratatui-core` + `ratatui-widgets` + a backend (`crossterm`). Correct for a
  full TUI, massively over-scoped for picking one task environment.
  Source: <https://crates.io/api/v1/crates/ratatui/0.30.2> (feature map).

**Recommendation: `inquire`, trimmed.** It gives the best multi-select UX for
the least bespoke code, and it is the only one of the three that itself forbids
`unsafe`. Because `MultiSelect` is a core prompt (not feature-gated), you can
keep it lean:

```toml
inquire = { version = "0.9", default-features = false, features = ["crossterm"] }
```

Use `default-features = false` to drop `macros`, `one-liners` and `fuzzy` (the
`fuzzy`/`one-liners` code paths are convenience only). If the picker should
never be compiled into a CI build, gate it behind an optional `tui` feature —
but note ticket 08 may also settle on "detect + confirm", in which case
`inquire`'s `Confirm`/`Select` covers that with the same dependency.

## 3. TOML round-trip editing — `toml_edit`

**Confirmed: `toml_edit` preserves comments, whitespace and relative item
order.** Its README: *"This crate allows you to parse and modify toml documents,
while preserving comments, spaces and relative order of items."* The canonical
entry point is **`DocumentMut`** — "The editable root TOML `Table`, containing
`Key`/`Value` pairs". Pattern: `let mut doc = s.parse::<DocumentMut>()?;`,
mutate via `doc["tool"]["momo"]["x"] = value(...)`, then `doc.to_string()`.
Version 0.25.17 is edition 2024 / MSRV 1.85. Default features are
`parse, display` (no `serde` needed).

Sources: <https://github.com/toml-rs/toml> (README),
<https://docs.rs/toml_edit/latest/toml_edit/struct.DocumentMut.html>,
<https://crates.io/api/v1/crates/toml_edit/0.25.17+spec-1.1.0>.

**Known limitations to design around** (from the same README): it does **not**
preserve *order of dotted keys* and does **not** preserve a *missing trailing
newline*. Neither matters for editing `mise.toml` / momo's own config, but if
momo ever writes a file it fully owns, don't assume byte-identical round-trip.

**Friction note:** if you enable the `serde` feature and serialise a struct back
out, you lose the formatting preservation that is the whole reason to use
`toml_edit`; for `mise.toml` edits, mutate the `DocumentMut` in place instead.
Plain `toml` (the serde facade) is *not* format-preserving and is the wrong
tool for the round-trip requirement.

## 4. Semver — `semver` crate + bump logic

**Use `semver` 1.0.** Zero runtime deps (optional `serde`), MSRV 1.68, ~232M
recent downloads, maintained by dtolnay, and it implements *Cargo's* flavour of
SemVer — the correct choice for a Rust-ecosystem tool
(<https://crates.io/crates/semver>, <https://github.com/dtolnay/semver>).

Key API facts (`semver` is a *parser/evaluator*, not a bump library — there is
no `bump_minor()`):

- Parse with `Version::parse("1.2.3-rc.1+build")?`.
- Fields: `major`, `minor`, `patch` (all `u64`), `pre: Prerelease`,
  `build: BuildMetadata`.
- Construct with `Version::new(major, minor, patch)` (empty pre/build) or the
  struct literal with `Prerelease::new("rc.1")?` / `BuildMetadata`.
- Ordering: `Version` implements `Ord` with SemVer precedence rules (pre-release
  sorts *before* the release; **build metadata is ignored** for precedence per
  <https://semver.org/#spec-item-10>).

So **momo owns the bump policy** as a small pure function, which is exactly what
the spec wants to make testable (tickets 01/05):

```text
MAJOR 0 : break|add -> (0, minor+1, 0), pre cleared ; fix -> (0, minor, patch+1)
MAJOR>=1: break -> (major+1,0,0) ; add -> (major, minor+1, 0) ; fix -> (major,minor,patch+1)
```

For RCs, build the version then attach a `Prerelease` (`1.3.0-rc.1` →
`-rc.2`), incrementing the numeric suffix yourself. Keep this in one module and
unit-test the edge table (`0.0.x`, multiple kinds per release) — the crate gives
you parse/compare/format, not the policy.

## 5. Dates/timestamps for changelog headings — `time` vs `chrono`

| | `time` 0.3.55 | `chrono` 0.4.45 |
|---|---|---|
| Edition / MSRV | 2024 / 1.88 | 2021 / 1.62 |
| Default features | `std` | `clock, std, oldtime, wasmbind` |
| Always-on deps | `deranged`, `num-conv`, `powerfmt`, `time-core` | `num-traits` |
| Extra deps when enabled | `libc`+`num_threads` (`local-offset`), `time-macros` (`macros`) | `iana-time-zone`, `windows-link` (`clock`/`winapi`), etc. |

Sources: <https://crates.io/api/v1/crates/time/0.3.55>,
<https://crates.io/api/v1/crates/chrono/0.4.45>,
<https://github.com/time-rs/time>.

**Recommendation: `time`, UTC, no extra features.** For a `## [1.2.0] - YYYY-MM-DD`
heading momo needs a current date and ISO-8601 rendering. `time::Date`
implements `Display` as ISO-8601, so:

```rust
let today = time::OffsetDateTime::now_utc().date(); // Date
let heading = format!("## [{version}] - {today}");   // {today} => 2026-10-12
```

`now_utc` and the `Display` impl come from the default `std` feature, so
`time = "0.3"` with **no** feature flags is enough — no `formatting`/`macros`,
no `libc`. (`local-offset`, for the machine's local date, would drag in `libc` +
`num_threads`; UTC is the right default for a changelog anyway.) `chrono` is a
fine crate with a shorter MSRV and a friendlier `Local::now().format("%F")`, but
it carries `iana-time-zone` and friends by default for timezone handling momo
doesn't need. `time` is the leaner, more constraint-aligned pick here.

## 6. Git access — `git2` vs shelling out to `git`

**Recommendation: shell out to the `git` CLI. Do not depend on `git2`.**

`git2` 0.21.0 is libgit2 bindings (edition 2021). Trade-offs against the repo's
"minimal deps / short but sweet" stance:

- **Heavy native build.** `git2` → `libgit2-sys` 0.18.4 → build-deps `cc` +
  `pkg-config` + `libz-sys`, with optional `libssh2-sys` / `openssl-sys`. The
  vendored libgit2 is a C build; the README explains it links vendored
  libgit2 unless a system libgit2 is found, and that network support needs the
  `https`/`ssh` features (which pull OpenSSL).
  Sources: <https://crates.io/api/v1/crates/git2/0.21.0/dependencies>,
  <https://docs.rs/libgit2-sys/latest/libgit2_sys/>,
  <https://github.com/rust-lang/git2-rs>.
- **`unsafe`/FFI.** `git2` is a wrapper over C (FFI is `unsafe` throughout).
  This does **not** break momo's `forbid` (lints are per-crate, see top), but it
  is exactly the kind of dependency a `forbid(unsafe_code)` project is signalling
  it wants to avoid — the pure-Rust, no-C-toolchain build is worth more than
  in-process libgit2 for momo's tiny tag/status surface.
- **Duplicate VCS logic.** momo must support **jj** too (ticket 12), and jj is
  only usable via its CLI. Implementing git via `git2` and jj via a subprocess
  means two different mental models for one "make a tag" operation. Shelling out
  to both gives one uniform "run a VCS command, parse stdout" seam behind the
  `TaskEnvironment`-style trait.
- **What momo actually needs from git is trivial**: detect a repo, check
  dirty/clean, create/list a tag. `git rev-parse`, `git status --porcelain`,
  `git tag`/`git tag --list` are stable, scriptable plumbing — no libgit2 needed.

Shelling out has its own costs (spawn overhead, parsing text, PATH/`git`
absent), but momo is a short-lived CLI, not a hot loop, so that's a non-issue.
Use `std::process::Command` (no `duct`/`xshell` needed) and put it behind a
trait so tests can inject a fake.

## 7. jj access — the `jj` CLI

Confirmed on this machine (`jj 0.46.0`) that jj has a first-class tag command —
**no library needed**:

```
jj tag set <NAMES>... -r/--revision <REVSET> [--allow-move]   # create/update
jj tag list [NAMES]...                                        # list
jj tag delete <NAMES>...
```

`jj tag set` defaults `-r` to `@`; `--allow-move` is required to move an
existing tag — useful for momo's "tag already exists" error path (ticket 06).
There is no `jj` Rust library with a stable API; the CLI is the supported
interface. Same `Command` seam as git. Source: local `jj tag set --help` /
`jj tag list --help` (jj 0.46.0); <https://jj-vcs.github.io/jj/latest/cli-reference/>.

Note for ticket 12: jj colocated repos are git-backed, but jj tags are jj-native
(`jj tag`), so momo's jj path should call `jj tag`, not `git tag`.

## 8. Errors — `anyhow` + `thiserror`

As mandated by `AGENTS.md`: **`anyhow` 1.0** at the top level (binary `main`,
`.context("why")`) and **`thiserror` 2.0** for typed library errors callers can
match on. Both dtolnay, both MSRV well under 1.99, both zero-dependency
(`thiserror` → `thiserror-impl` proc-macro only). Sources:
<https://crates.io/crates/anyhow>, <https://crates.io/crates/thiserror>.

Suggested shape: `thiserror` enum for the domain (`Error::NoVcs`,
`Error::TagExists`, `Error::DirtyTree`, `Error::NothingToRelease`) so the CLI
can map variants to exit codes/messages; `anyhow` for I/O and `?`-propagation
at the edges. Keep it small — one typed enum, not one per module.

---

## Recommended `Cargo.toml` (dependencies)

```toml
[dependencies]
anyhow = "1.0"                                        # top-level error context
clap = { version = "4.6", features = ["derive"] }     # CLI
semver = "1.0"                                        # parse/compare versions
thiserror = "2.0"                                     # typed domain errors
time = "0.3"                                          # UTC date for headings
toml_edit = "0.25"                                    # comment-preserving TOML

# Interactive task-environment picker (ticket 08). Trimmed; drop entirely if
# ticket 08 lands on flags/detect-only, or gate behind a `tui` feature.
inquire = { version = "0.9", default-features = false, features = ["crossterm"] }
```

No `git2`, no `chrono`, no `ratatui`, no `dialoguer`, no `argh`/`lexopt`.

### Dev-dependencies

```toml
[dev-dependencies]
tempfile = "3"     # scratch dirs/files for changelog + config tests
assert_cmd = "2"   # CLI integration tests (binary invoked end-to-end)
predicates = "3"   # assertions on CLI stdout/stderr for assert_cmd
```

(`assert_cmd` + `predicates` are the standard pair for testing a clap CLI
end-to-end; `tempfile` covers filesystem tests. All are pure-Rust and
dev-only, so they don't affect the shipped dependency footprint.)

---

## Constraint checks / friction

- **`unsafe_code = "forbid"`** — satisfied. The lint is per-crate and does not
  see dependencies, so nothing here *breaks* the build. Choosing shell-out over
  `git2` keeps momo's own tree free of FFI and avoids a C toolchain.
- **Edition 2024** — momo's own edition is independent of its deps' editions.
  `clap` 4.6, `toml_edit` 0.25 and `time` 0.3 are themselves edition 2024; the
  rest are 2018/2021. No issue.
- **MSRV 1.99** — comfortably above every candidate's MSRV (max 1.88 for
  `time`/`ratatui`; `git2` declares none). No issue.
- **Clippy warnings-as-errors** — no candidate is known to emit clippy lints
  into *our* crate; derive macros (`clap`, `thiserror`) expand in our crate but
  produce conventional code. If a generated impl ever trips a pedantic lint,
  scope an `#[expect(...)]` with a comment (per `AGENTS.md`), don't add a
  blanket `#[allow]`.
- **Minimal deps** — the recommended set is 6 runtime crates (7 with the
  picker). The single biggest weight decision is **not** pulling `git2`
  (native C build) and **not** pulling `ratatui` (full TUI framework) for a
  one-off prompt.
- **Open question for ticket 08:** if the picker is dropped or gated, remove
  `inquire` from the default dependency set so CI/standalone builds stay lean.
