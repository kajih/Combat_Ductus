# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project state

Past the early-stage/empty-window point — a real client/server versus fighter with two networked players, per `CONTEXT.md`'s domain model and the ADRs/PRDs in `docs/`. Two binaries share one library crate:

- **`client`** (`src/main.rs`, the default binary) — `DefaultPlugins`, built both as the fast native dev build and as the Trunk/wasm web build. Pure renderer per ADR 0005: never simulates, only reacts to server snapshots.
- **`server`** (`src/bin/server.rs`) — headless (`MinimalPlugins`, no rendering), native-only (`#[cfg(not(target_arch = "wasm32"))]` — a browser tab can't bind a listening socket). Owns the one authoritative `combat::MatchState` and steps it every tick regardless of who's connected.

Shared library modules (`src/lib.rs`, used by both binaries):
- **`combat`** — the deep module: pure Rust, no Bevy/networking dependency. Health, damage, range checks, Jump's arc, Special's gating and cooldown, Match-end/winner detection. The only module carrying unit tests.
- **`net_protocol`** — shared wire types (`InputEvent`, `StateSnapshot`, `MatchStatus`), plain serde structs/enums, JSON over WebSocket. Compiles identically on `wasm32-unknown-unknown` and native.
- **`server_net`** (native-only) — the server binary's guts: WebSocket accept loop (per-connection player-slot assignment, spectator fallback, disconnect handling) on a small tokio runtime, plus the Bevy app that steps `combat::MatchState` on a fixed ~30Hz schedule and broadcasts snapshots.
- **`client_net`** — the client's WebSocket connection wrapper (`ewebsock`), compiles on both targets with one poll-based API.

Client-only modules (`src/*.rs`, wired up in `main.rs`):
- **`connect_screen`** — the Connect screen UI, connection lifecycle, the `AppState` state machine (`Connecting` → `InMatch` → `MatchEnded`), and the `LatestSnapshot` resource everything else reads from.
- **`character_rig`** — composites a Character at runtime from layered torso/arm/leg/face sprite entities per (Body Type, Facing), per ADR 0007/0006.
- **`match_characters`** — spawns/positions both Characters from server snapshots, drives Punch/Kick limb-swing poses and the Motivational Speech bubble.
- **`health_hud`** — plain Health readout overlay, no polish by design.
- **`match_end_screen`** — the "X wins" screen and its restart control.
- **`input`** — captures A/D/Space/J/K/H and forwards them to the server as `InputEvent`s; never simulates locally.
- **`stage`** — the single fixed Stage backdrop and its static camera.
- **`asset_diagnostics`** — logs image asset load success/failure (a first slice of `docs/issues/observability/logging.md`).

Update this section again once the architecture changes meaningfully (a new milestone, a new module boundary) rather than letting it drift stale.

## Git workflow & issue tracking

- Work lives on GitHub: `origin` is `git@github.com:kajih/Combat_Ductus.git` (the repo was renamed from `Ductus_Combat` at one point — if a push ever reports the old name via a redirect, update the remote URL to match rather than relying on the redirect).
- Branch before committing new work — don't commit directly to `main`. Push the branch and open a PR (`gh pr create`). A stacked branch (built on top of another not-yet-merged branch) should have its PR base set to that branch, not `main`, until the earlier one merges — GitHub auto-retargets the base to `main` once the branch it was pointed at gets merged/deleted.
- Every filed issue is a markdown file under `docs/issues/<category>/<slug>.md` (`Parent`/`What to build`/`Acceptance criteria`/`Blocked by`) — this is the source of truth for design decisions and reasoning, not just a checklist.
- Issues are also mirrored to GitHub Issues (https://github.com/kajih/Combat_Ductus/issues), one per file, labeled by their `docs/issues/` subfolder (`combat-foundation`, `audio`, `character-select`, `observability`, `spectator-mode`, `tooling` — create a new matching label if a new subfolder shows up). Each synced local file gets a `## GitHub Issue` footer linking to its GitHub counterpart; check for that footer before re-syncing a file, so a re-sync doesn't create duplicates.
- `gh` (GitHub CLI) is authenticated on this machine and is the expected way to create/manage PRs and issues.

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
