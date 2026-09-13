## Parent

None - dev tooling, not part of any milestone PRD.

## What to build

Add a Makefile at the repo root covering the build/clean workflows currently documented as separate manual commands in `CLAUDE.md`/`README.md`: the fast native dev client (`cargo run --features dev`), a plain release-equivalent native build (`cargo build`), the server binary, and the production web build (`trunk build --release`) - plus a `clean` target that removes build output so a subsequent build is genuinely from scratch. A nice-to-have: a target that runs `trunk serve` directly, so starting local web dev is one command instead of remembering the separate `trunk` invocation.

**Decided target behavior** (resolved via design review):
- Bare `make` (no target given) prints a `make help`-style listing of available targets and does nothing else - no surprise side effects from an unqualified `make`.
- A `make all` target exists, covering every *non-running* build artifact: the native release build, the server build, and `trunk build --release`. It deliberately excludes `dev` and `web` (`trunk serve`), since those launch/serve rather than just build, and "build everything" shouldn't also start a long-running dev server.
- `make clean` removes both `target/` (cargo's build cache) and `dist/` (trunk's gitignored web output) - a "genuinely from scratch" clean should cover every build artifact this project produces, not just the native side.
- Cross-platform by default - the primary dev environment for this project is Linux, where `make` is already standard; this machine is Windows only incidentally (WSL's graphics support is unreliable for a Bevy window, so this session runs natively on Windows instead). `make` isn't installed by default on Windows and needs calling out as a one-line prerequisite (e.g. via chocolatey - `choco install make` - already how it's available on this machine), the same way `cargo-binutils`/`llvm-tools-preview` already are in `CLAUDE.md`. Nothing in the Makefile itself should assume Windows - the existing Windows-only build tooling (`.cargo/config.toml`'s linker pin) already degrades gracefully on its own if absent, and this shouldn't need anything analogous.

Remaining open branch: exact target names beyond `help`/`all`/`clean` (e.g. `dev`, `build`, `server`, `web`, `web-release`) - left to whoever implements this, naming is low-stakes relative to the behavior already decided above.

## Acceptance criteria

- [x] Bare `make` prints available targets and takes no other action
- [x] A target builds and can run the fast native dev client (`--features dev`)
- [x] A target produces the release-equivalent native build (plain `cargo build`)
- [x] A target builds/runs the server binary
- [x] A target runs `trunk serve` for local web development
- [ ] `make all` builds every non-running artifact (native release, server, `trunk build --release`) without starting `dev` or `web`
- [x] `make clean` removes both `target/` and `dist/` so a subsequent build starts genuinely from scratch
- [x] The Makefile and its targets are documented in `CLAUDE.md`/`README.md` alongside (not replacing) the existing plain-cargo commands, since those remain valid on a machine without `make`
- [x] Nothing in the Makefile assumes Windows; the one Windows-specific note is `make`'s own installation prerequisite, documented alongside the project's existing Windows-only setup notes

**`make all` left unchecked**: `make build`, `make server`, `make dev`, `make web`, and `make clean` were all verified working live on this machine. `make web-release` (and so `make all`, which depends on it) currently fails on this machine partway through `trunk build --release`'s `wasm-opt` post-processing step (`error copying (optimized) wasm file to dist dir: The system cannot find the path specified. (os error 3)`). This reproduces with the bare `trunk build --release` command too, unchanged by this issue - it's a pre-existing environment/tooling issue on this Windows machine, not a regression introduced by the Makefile. Worth its own tooling issue if it turns out to affect other machines too.

## Blocked by

None - purely additive tooling, doesn't depend on any unbuilt feature.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/26
