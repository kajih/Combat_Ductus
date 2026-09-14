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

Builds for `wasm32-unknown-unknown`, serves on `http://localhost:8080`, opens a browser tab, and rebuilds automatically on file changes. To play with someone else, see [Playing over the LAN](#playing-over-the-lan) below.

For a real, optimized build (much smaller than the `trunk serve` dev output, which is 100MB+ and expected to be that large): `trunk build --release`, output goes to `dist/` (gitignored — never commit it).

### The client, natively (faster iteration while developing)

```
cargo run --features dev
```

`--features dev` enables Bevy's dynamic linking, which cuts incremental rebuild times drastically. **Never** use it for a release build (it requires shipping `bevy_dylib` alongside the binary) or for the web build (dynamic linking doesn't apply to `wasm32-unknown-unknown` at all). A plain `cargo build` produces a release-equivalent, statically-linked binary.

## Playing over the LAN

Both players load the client from the **host** machine — the one running the server binary and the Trunk dev server. Nothing needs installing on the other machine; it just needs a browser.

On the host:

1. **Start the server** (it must stay running — if it exits, the Connect screen just times out):
   ```
   cargo run --bin server
   ```
   It listens on `0.0.0.0:9000`, i.e. all interfaces.

2. **Serve the client**:
   ```
   trunk serve
   ```
   `Trunk.toml` sets `address = "0.0.0.0"` so this is reachable from other machines. Trunk binds **loopback only** by default, which is the single easiest way to end up with a dev server nobody else can reach.

3. **Find the host's LAN IP**:
   ```
   ip -4 -br addr        # Linux
   ipconfig              # Windows
   ```

4. **Open the firewall** for ports 8080 (the page) and 9000 (the game). On Linux with `ufw`, scoped to the LAN rather than the world:
   ```
   sudo ufw allow from 192.168.0.0/16 to any port 8080 proto tcp
   sudo ufw allow from 192.168.0.0/16 to any port 9000 proto tcp
   ```

On the other machine, browse to `http://<host-LAN-IP>:8080`. The Connect screen pre-fills the address with the host that served the page, so it should already read `<host-LAN-IP>:9000` — just hit Connect.

### When it doesn't work

A **timeout** (rather than a refused connection) almost always means a packet is being dropped rather than a service being down. Work outwards from the host:

| Check | Command (on the host) | Expect |
| --- | --- | --- |
| Server is running | `ss -tln \| grep 9000` | `0.0.0.0:9000` |
| Trunk is serving beyond loopback | `ss -tln \| grep 8080` | `0.0.0.0:8080`, **not** `127.0.0.1:8080` |
| Firewall | `sudo ufw status verbose` | 8080 and 9000 allowed |
| Basic reachability | `ping <host-IP>` *from the other machine* | replies |

Two traps worth knowing, both of which cost real time once:

- **Trunk's `addresses` (plural) key is silently ignored.** It logs a completely normal `server listening at ...` banner listing every LAN IP while binding nothing at all. Use the singular `address`, and trust `ss`, not the banner.
- **A host with two interfaces on overlapping subnets** can reply out the wrong one, which also looks like a timeout. `ip route` shows which interface wins for a given destination.

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
