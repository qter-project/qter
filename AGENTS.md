# AGENTS.md

Guidance for OpenCode agents working in this repo. Verified against current config; where docs conflict with code, code wins.

## Environment

- The Cargo workspace lives at `src/`, **not** the repo root. Run cargo from `src/`, or pass `--manifest-path src/Cargo.toml`. The repo root holds `flake.nix`, docs, and hardware files.

## Workspace crates

`src/Cargo.toml` uses `members = ["*"]` (excludes `.zed`, `.config`, `target`):

- `cli` — the `qter` binary. Entry: `src/cli/src/main.rs`.
- `compiler` — QAT -> IR and IR -> Q. Parses with **chumsky** (not pest); procedural macros run via **Rhai** (not Lua).
- `qter_core` — shared types, math, and the IR/runtime used by `compiler` + `interpreter`.
- `interpreter` — executes the IR. Has a `remote_robot` feature (used by `robot` and `visualiser`).
- `cycle_combination_finder` (CCF), `cycle_combination_solver` (CCS) — math-heavy architecture search; see testing notes.
- `movecount_coefficient_calculator`, `g4g_program`, `robot`, `visualiser`.
- Several crates depend on qter-project org git repos (`puzzle_theory`, `pog_ans`, `bitgauss`, `union-find`); cargo fetches them automatically.

## CLI

- Run: `cargo run -p cli -- interpret file.qat [-t]...` (trace level 0-3 via repeated `-t`). `compile file.qat` emits `file.q`. Interpreting `.q` files is still `todo!()` and will panic.
- `debug`, `test`, `compress`, `dump` subcommands are gated by `#[cfg(debug_assertions)]` — only present in debug builds; they vanish under `--release`.

## Building / testing

- Tests use **nextest** (config `src/.config/nextest.toml`). Custom profiles: `ccs`, `ccf`. The `ccf` profile sets `fail-fast = false` and pins `threads-required = num-cpus` for `possible_orders::puzzle` and `finder::tests`.
- Run a single test: `cargo nextest run -p <crate> <name>` (fallback: `cargo test -p <crate> <name>`).
- Test crates use `test-log` + `pretty_env_logger` for tracing output.
- `cargo fmt` runs nightly rustfmt. The CCF and CCS crates have their own `rustfmt.toml` with unstable options (`imports_granularity = "Crate"`, `group_imports = "StdExternalCrate"`, etc.).
- Clippy (workspace lints): `disallowed_types = "deny"` — `pest::error::ErrorVariant` is banned (use `qter_core`'s error helper). `type_complexity` is allowed. `src/clippy.toml` exempts `qter_core::WithSpan` from the interior-mutability lint.

## Robot (Raspberry Pi target)

- `src/robot/.cargo/config.toml` pins `build.target = "aarch64-unknown-linux-gnu"`, so building in that crate cross-compiles for the RPi. It uses `rppal` (RPi GPIO) and runs only on a Pi.
- Runtime needs the `twophase` binary on PATH. On first run it generates large `.tbl` pruning tables into a cache dir. All `*.tbl` are gitignored — never commit them.
- Deploy: `src/robot/push.sh` (release build + scp).

## Visualiser (wasm)

- Build with `src/visualiser/build.sh` (runs `npm install`, clones `tree-sitter-q`, builds the tree-sitter wasm, `wasm-pack build --target web --out-dir dist`, and `tsc`). Do **not** use plain `cargo build` for this crate.
- Serve locally with `build.sh caddy-serve` (Caddy serving `dist`). Other subcommands: `tsc-watch`, `wasm-pack`, `cp-static`, `clean`.
- Crate type is `cdylib` for `wasm32-unknown-unknown`; uses the `interpreter` `remote_robot` feature.

## Docs

- Book: `shiroa build docs --mode static-html` (output `docs/dist`). Paper: `typst compile paper/paper.typ` from the `media/` directory. Both redeploy on push to `main` via `.github/workflows/deploy.yml`.

## Rules

- When instructions are ambiguous or inconsistent, ask the user for clarification. If the ambiguity might be resolved by reading code, do that first.
- Ask the user about architectural decisions
