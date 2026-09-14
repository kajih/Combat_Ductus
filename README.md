# Combat Ductus

An internal-only 2D versus fighting game, built as a demo/showcase of AI-assisted development. Playable characters are stylized depictions of the company's own employees (managers and up). See [`CONTEXT.md`](CONTEXT.md) for the full domain glossary and [`docs/adr/`](docs/adr/) for the design decisions behind it.

It's a client/server game: a native **server** binary runs the one authoritative Match simulation, and every player — including whoever hosts — connects to it as a **client** running in a web browser. It's built for play over a local network (office LAN), not the internet.

## Prerequisites

- **Rust**, edition 2024 (a reasonably recent stable or nightly toolchain — install via [rustup](https://rustup.rs/) if you don't have it)
- The **`wasm32-unknown-unknown`** target, for the browser build:
  ```
  rustup target add wasm32-unknown-unknown
  ```
  Note that rustup targets are installed **per toolchain**, not per machine — if you switch your default toolchain (say from stable to nightly), run this again for the new one, or the web build fails with `can't find crate for 'core'` / `the wasm32-unknown-unknown target may not be installed`.
- **[Trunk](https://trunkrs.dev/)**, the tool that builds and serves the browser client:
  ```
  cargo install trunk
  ```
  Trunk downloads `wasm-bindgen` and `wasm-opt` into its own cache on first use, so those don't need installing separately.

That's everything required. The remaining setup is optional build-speed tuning (see [`CLAUDE.md`](CLAUDE.md) for details) — skip it and the project still builds fine, just slower to iterate on. `.cargo/config.toml` swaps in a faster linker per platform, so install whichever matches yours:

- **Linux**: `clang` and `mold` (on Arch, `pacman -S clang mold`).
- **Windows**: `rustup component add llvm-tools-preview` and `cargo install cargo-binutils`, for `rust-lld.exe`.

If you have neither, delete `.cargo/config.toml` to fall back to the default linker.

## Building and running

The crate has **two binaries**: the client (`combat-ductus`, the default) and the headless server (`server`). You need both running to actually play.

### The server

The server never renders anything — it's a plain terminal process:

```
cargo run --bin server
```

By default it listens on `0.0.0.0:9000`. Override with the `COMBAT_DUCTUS_SERVER_ADDR` environment variable, e.g.:

```
COMBAT_DUCTUS_SERVER_ADDR=127.0.0.1:9000 cargo run --bin server
```

### The client, in a browser (the primary target)

```
trunk serve
```

Builds for `wasm32-unknown-unknown`, serves on `http://localhost:8080`, opens a browser tab, and rebuilds automatically on file changes. Open that URL (or share the host machine's LAN IP with other players on the same network), type in the server's address on the Connect screen, and play.

For a real, optimized build (much smaller than the `trunk serve` dev output, which is 100MB+ and expected to be that large): `trunk build --release`, output goes to `dist/` (gitignored — never commit it).

### The client, natively (faster iteration while developing)

```
cargo run --features dev
```

`--features dev` enables Bevy's dynamic linking, which cuts incremental rebuild times drastically. **Never** use it for a release build (it requires shipping `bevy_dylib` alongside the binary) or for the web build (dynamic linking doesn't apply to `wasm32-unknown-unknown` at all). A plain `cargo build` produces a release-equivalent, statically-linked binary.

## Testing and linting

```
cargo test        # run the test suite (combat rules, networking, rendering-support logic)
cargo clippy      # lint
cargo fmt         # format
```

## Makefile

A root `Makefile` wraps the commands above as `make` targets, for anyone with `make` on their machine (Linux/macOS have it by default; on Windows, install it separately, e.g. `choco install make`). It's purely a convenience layer — every plain cargo/trunk command above still works and is the only option without `make`.

Run `make` with no target (or `make help`) to list what's available:

```
make dev          # cargo run --features dev
make build        # cargo build (both the client and server binaries)
make server       # cargo run --bin server
make web          # trunk serve
make web-release  # trunk build --release
make all          # every non-running build artifact: build + web-release
make clean        # remove target/ and dist/, for a genuinely from-scratch build
```

## Learn more

- [`CONTEXT.md`](CONTEXT.md) — the project's domain glossary (Character, Match, Facing, Special, etc.)
- [`docs/adr/`](docs/adr/) — architecture decision records explaining *why* the game works the way it does
- [`docs/prd/`](docs/prd/) — the product requirements doc for the current milestone
- [`docs/issues/`](docs/issues/) — the tracked backlog of vertical-slice work
- [`CLAUDE.md`](CLAUDE.md) — day-to-day commands and the fast-compile setup, for anyone (human or AI) working on the code
