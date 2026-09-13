## Parent

None - dev tooling, not part of any milestone PRD.

## What to build

Add a Makefile at the repo root covering the build/clean workflows currently documented as separate manual commands in `CLAUDE.md`/`README.md`: the fast native dev client (`cargo run --features dev`), a plain release-equivalent native build (`cargo build`), the server binary, and the production web build (`trunk build --release`) - plus a `clean` target that removes build output so a subsequent build is genuinely from scratch. A nice-to-have: a target that runs `trunk serve` directly, so starting local web dev is one command instead of remembering the separate `trunk` invocation.

Concretely, at minimum this should settle:
- Exact target names (e.g. `dev`, `build`, `server`, `web`, `web-release`, `clean`, `all`) and what the bare `make` (no target) default does
- Whether `make all` builds every variant in one pass, or that's deliberately left out (a full rebuild of every variant could be slow and isn't necessarily a common workflow)
- What exactly `clean` removes - `target/` alone, or `target/` and `dist/` (the web build's gitignored output directory) both
- Cross-platform by default - the primary dev environment for this project is Linux, where `make` is already standard; this machine is Windows only incidentally (WSL's graphics support is unreliable for a Bevy window, so this session runs natively on Windows instead). `make` isn't installed by default on Windows and needs calling out as a one-line prerequisite (e.g. via chocolatey - `choco install make` - already how it's available on this machine), the same way `cargo-binutils`/`llvm-tools-preview` already are in `CLAUDE.md`. Nothing in the Makefile itself should assume Windows - the existing Windows-only build tooling (`.cargo/config.toml`'s linker pin) already degrades gracefully on its own if absent, and this shouldn't need anything analogous

## Acceptance criteria

- [ ] A target builds and can run the fast native dev client (`--features dev`)
- [ ] A target produces the release-equivalent native build (plain `cargo build`)
- [ ] A target builds/runs the server binary
- [ ] A target runs `trunk serve` for local web development
- [ ] A `clean` target removes build output (`target/`, and `dist/` if applicable) so a subsequent build starts genuinely from scratch
- [ ] The Makefile and its targets are documented in `CLAUDE.md`/`README.md` alongside (not replacing) the existing plain-cargo commands, since those remain valid on a machine without `make`

## Blocked by

None - purely additive tooling, doesn't depend on any unbuilt feature.
