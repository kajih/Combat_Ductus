# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project state

This is an early-stage Rust/Bevy project. `src/main.rs` currently just opens an empty window (`App::new().add_plugins(DefaultPlugins).run()`) — no game code, ECS setup, or module structure exists yet. There is no architecture to describe until real code is added; update this file once the project takes shape (e.g. plugin structure, module layout, component/resource design).

## Commands

- **Run in dev (fast iteration, use this day-to-day): `cargo run --features dev`**
  Enables Bevy's dynamic linking, cutting incremental build times drastically (seconds vs. minutes). Never use `--features dev` for release builds — it requires shipping `bevy_dylib` alongside the binary.
- Build (release-equivalent, no dynamic linking): `cargo build`
- Check (fast compile check without producing a binary): `cargo check`
- Test: `cargo test`
- Run a single test: `cargo test <test_name>`
- Lint: `cargo clippy`
- Format: `cargo fmt`

Note: this project uses Rust edition 2024 and Bevy 0.19.1 — when adding code, match APIs to that Bevy version (Bevy's API changes significantly between minor versions).

## Web (WASM) build

- **Run in browser (dev, fast iteration): `trunk serve`** — builds for `wasm32-unknown-unknown`, serves on `http://localhost:8080` and opens a browser tab, and rebuilds on file changes.
- Production web build: `trunk build --release` — output goes to `dist/` (wasm-opt'd, much smaller than the dev build; a plain `trunk build`/`trunk serve` dev build is large — 100MB+ unoptimized `.wasm` with debug info — that's expected).
- `index.html` is the Trunk entry point (`<link data-trunk rel="rust" .../>` tells Trunk to build this crate); `Trunk.toml` configures the dist dir and dev server.
- `dist/` is build output, gitignored — never commit it.
- Never pass `--features dev` for web builds — dynamic linking doesn't apply/work on wasm32.

Requires the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`) and `trunk` (`cargo install trunk`) — both already set up on this machine.

**getrandom on wasm32-unknown-unknown**: `rand`/`uuid` pull in `getrandom`, which has no OS RNG source on wasm32-unknown-unknown and fails to compile there without help. `Cargo.toml` adds a target-specific `getrandom` dependency with the `wasm_js` feature enabled (routes randomness through the browser's Web Crypto API) — see `[target.'cfg(target_arch = "wasm32")'.dependencies]`. Don't remove this or wasm builds will fail with a `compile_error!` from `getrandom`.

## Fast-compile setup (already configured)

This repo is set up per Bevy's official Windows fast-compiles guide:

- **`[features] dev = ["bevy/dynamic_linking"]`** in `Cargo.toml` — opt-in dynamic linking (see Commands above).
- **`[profile.dev]` split** in `Cargo.toml` — our own crate compiles at `opt-level = 1` (fast, unoptimized) while all dependencies, including Bevy, compile at `opt-level = 3` (needed both for dynamic linking to work on Windows and because a fully-unoptimized Bevy is too slow at runtime to be usable).
- **`.cargo/config.toml`** — pins the linker to `rust-lld.exe` for `x86_64-pc-windows-msvc`, which is significantly faster than the default MSVC linker. Requires the `llvm-tools-preview` rustup component and `cargo-binutils` (both already installed on this machine via `rustup component add llvm-tools-preview` and `cargo install cargo-binutils`) — if setting up a fresh machine, reinstall those two first.

If cloning onto a machine without `rust-lld.exe` on `PATH`, either install the two components above or delete `.cargo/config.toml` to fall back to the default linker (slower, but no extra setup).
