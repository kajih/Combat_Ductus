# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project state

Past the early-stage/empty-window point — a real client/server versus fighter with two networked players, per `CONTEXT.md`'s domain model and the ADRs/PRDs in `docs/`. Two binaries share one library crate:

- **`client`** (`src/main.rs`, the default binary) — `DefaultPlugins`, built both as the fast native dev build and as the Trunk/wasm web build. Pure renderer per ADR 0005: never simulates, only reacts to server snapshots.
- **`server`** (`src/bin/server.rs`) — headless (`MinimalPlugins`, no rendering), native-only (`#[cfg(not(target_arch = "wasm32"))]` — a browser tab can't bind a listening socket). Owns the one authoritative `combat::MatchState` and steps it every tick regardless of who's connected.

Shared library modules (`src/lib.rs`, used by both binaries):
- **`combat`** — the deep module: pure Rust, no Bevy/networking dependency. Health, damage, range checks, Jump's arc, Special's gating and cooldown, Match-end/winner detection (including a forfeit's `MatchEndReason` alongside a health-depleted one). The only module whose tests are pure unit tests, with no server or socket behind them — `src/combat.rs` is the production code, `src/combat/tests.rs` the test module (a folder module, not a separate top-level module — everything is still `crate::combat::*`).
- **`net_protocol`** — shared wire types (`InputEvent`, `StateSnapshot`, `MatchStatus`), plain serde structs/enums, JSON over WebSocket. Compiles identically on `wasm32-unknown-unknown` and native.
- **`server_net`** (native-only) — the server binary's guts: WebSocket accept loop (per-connection player-slot assignment, spectator fallback, disconnect/forfeit handling) on a small tokio runtime, plus the Bevy app that steps `combat::MatchState` on a fixed ~30Hz schedule and broadcasts snapshots. A folder module: `src/server_net.rs` is the slim root (shared types, `mod` declarations, and the only two re-exported public functions), `src/server_net/accept.rs` the accept loop, `src/server_net/simulation.rs` the Bevy app/systems, `src/server_net/tests.rs` the integration test suite (driven over real WebSocket connections against a real local server) — all still `crate::server_net::*`, never separate top-level modules.
- **`client_net`** — the client's WebSocket connection wrapper (`ewebsock`), compiles on both targets with one poll-based API.

Client-only modules (`src/*.rs`, wired up in `main.rs`):
- **`connect_screen`** — the Connect screen UI, connection lifecycle, the `AppState` state machine (`Connecting` → `InMatch` → `MatchEnded`), and the `LatestSnapshot` resource everything else reads from. A folder module: `src/connect_screen.rs` is the production code, `src/connect_screen/tests.rs` its lifecycle tests, which drive the real plugin on a headless Bevy app against a real local server.
- **`role_indicator`** — a small on-screen indicator of which slot (P1/P2/spectator) this connection holds, resetting on disconnect.
- **`character_rig`** — composites a Character at runtime from layered torso/arm/leg/face sprite entities per (Body Type, Facing), per ADR 0007/0006.
- **`match_characters`** — spawns/positions both Characters from server snapshots, drives Punch/Kick limb-swing poses and the Motivational Speech bubble.
- **`health_hud`** — plain Health readout overlay, no polish by design.
- **`match_end_screen`** — the "X wins" (or "X wins - Y disconnected!" for a forfeit) screen and its restart control, restricted server-side to a real player slot.
- **`input`** — captures A/D/Space/J/K/H and forwards them to the server as `InputEvent`s; never simulates locally.
- **`stage`** — the single fixed Stage backdrop and its static camera.
- **`asset_diagnostics`** — logs image asset load success/failure (a first slice of `docs/issues/observability/logging.md`).

A module that outgrows one file this way (production code dwarfed by its own test suite, or itself sprawling) becomes a folder module — the file keeps the module's name (`server_net.rs`, not `mod.rs`) and stays the slim root; the pieces live alongside it in a same-named folder. External code never sees the difference: only the root re-exports anything, so `server_net::spawn_network_thread` etc. stay exactly as callable as before the split. Prefer this over actually splitting a module's *responsibilities* into separate top-level modules unless they're genuinely independent concerns.

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

### Makefile

A root `Makefile` wraps the commands above (and the web-build ones below) as `make` targets — run `make` (or `make help`) with no target for the full list (`dev`, `build`, `server`, `web`, `web-release`, `all`, `clean`). This is purely a convenience layer: the plain cargo/trunk commands documented throughout this file remain valid and are the only option on a machine without `make`. `make` isn't installed by default on Windows — this machine has it via `choco install make`; the Makefile itself doesn't assume Windows in any other way.

## Web (WASM) build

- **Run in browser (dev, fast iteration): `trunk serve`** — builds for `wasm32-unknown-unknown`, serves on `http://localhost:8080` and opens a browser tab, and rebuilds on file changes.
- Production web build: `trunk build --release` — output goes to `dist/` (wasm-opt'd, much smaller than the dev build; a plain `trunk build`/`trunk serve` dev build is large — 100MB+ unoptimized `.wasm` with debug info — that's expected).
- **LAN play**: `Trunk.toml` sets `[serve] address = "0.0.0.0"` so other machines on the LAN can load the client — Trunk otherwise binds loopback only. Use the singular `address` key: the `addresses` list form is silently ignored by trunk 0.21.14, which then logs a normal "server listening at" banner (LAN IPs and all) while binding nothing. Trust `ss -tln | grep 8080`, not the banner.
- `index.html` is the Trunk entry point (`<link data-trunk rel="rust" .../>` tells Trunk to build this crate); `Trunk.toml` configures the dist dir and dev server.
- `dist/` is build output, gitignored — never commit it.
- Never pass `--features dev` for web builds — dynamic linking doesn't apply/work on wasm32.

Requires the `wasm32-unknown-unknown` target (`rustup target add wasm32-unknown-unknown`) and `trunk` (`cargo install trunk`) — both set up on this machine. Trunk fetches `wasm-bindgen` and `wasm-opt` into its own cache on first use; they don't need to be on `PATH`.

**rustup targets are per toolchain**: if the default toolchain changes (this machine defaults to `nightly`, and also has `stable` and `esp` installed), the wasm target has to be added again for the new one. Otherwise the web build — and `make all` with it — fails partway through dependency compilation with `error[E0463]: can't find crate for 'core'` and `note: the wasm32-unknown-unknown target may not be installed`. Check with `rustup target list --installed` before assuming anything subtler; `rustup +stable target list --installed` shows a *different* set.

**Known-working `trunk` version on Windows**: `trunk build --release`'s `wasm-opt` post-processing step previously failed on this machine with `error copying (optimized) wasm file to dist dir: The system cannot find the path specified. (os error 3)` (root cause never conclusively diagnosed — see `docs/issues/tooling/trunk-build-release-fails-on-windows.md`). Confirmed working, reproducibly, with `trunk 0.21.14`. If this resurfaces on a different `trunk` version, that's a signal it may in fact be version-specific after all.

**getrandom on wasm32-unknown-unknown**: `rand`/`uuid` pull in `getrandom`, which has no OS RNG source on wasm32-unknown-unknown and fails to compile there without help. `Cargo.toml` adds a target-specific `getrandom` dependency with the `wasm_js` feature enabled (routes randomness through the browser's Web Crypto API) — see `[target.'cfg(target_arch = "wasm32")'.dependencies]`. Don't remove this or wasm builds will fail with a `compile_error!` from `getrandom`.

## Fast-compile setup (already configured)

This repo is set up per Bevy's official fast-compiles guide:

- **`[features] dev = ["bevy/dynamic_linking"]`** in `Cargo.toml` — opt-in dynamic linking (see Commands above).
- **`[profile.dev]` split** in `Cargo.toml` — our own crate compiles at `opt-level = 1` (fast, unoptimized) while all dependencies, including Bevy, compile at `opt-level = 3` (needed both for dynamic linking to work on Windows and because a fully-unoptimized Bevy is too slow at runtime to be usable).
- **`.cargo/config.toml`** — replaces the default platform linker, which is the single biggest cost in Bevy's incremental builds. It carries one block per platform, and cargo only applies the one matching the host target, so both can stay checked in as the project moves between machines:
  - `x86_64-unknown-linux-gnu` — `clang` as the linker driver with **mold** doing the actual linking (`-Clink-arg=-fuse-ld=mold`). Needs `clang` and `mold` on `PATH` (Arch: `pacman -S clang mold`; both already installed on this machine). Confirm it took effect with `readelf -p .comment target/debug/server | grep mold`.
  - `x86_64-pc-windows-msvc` — `rust-lld.exe`, significantly faster than the default MSVC linker. Requires the `llvm-tools-preview` rustup component and `cargo-binutils` (`rustup component add llvm-tools-preview`, `cargo install cargo-binutils`).

If cloning onto a machine that has neither the platform's fast linker nor its prerequisites, either install them or delete `.cargo/config.toml` to fall back to the default linker (slower, but no extra setup).
