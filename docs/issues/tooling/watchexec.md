## Parent

None - dev tooling, builds on `docs/issues/tooling/makefile-build-targets.md`'s Makefile.

## What to build

Consider using `watchexec` to auto-rebuild/restart the server and/or the client during development, so editing source doesn't require manually killing and re-running `cargo run --bin server` / `cargo run --features dev` each time - matching what `trunk serve` already gives the web client for free (rebuild + reload on file change).

Likely shape: one or more new Makefile targets wrapping `watchexec` around the existing `cargo run` commands, watching `src/` (and `Cargo.toml`) and restarting the relevant binary on change. Requires `watchexec` (the `watchexec-cli` crate - `cargo install watchexec-cli` - or a package manager install) as a new documented prerequisite, the same way `trunk`/`cargo-binutils` already are in `CLAUDE.md`.

Remaining open branches (not yet decided):
- Whether this covers the server only, the native client only, or both.
- Exact target name(s) and invocation (e.g. `make watch-server`).
- Debounce/ignore-pattern tuning - `watchexec` respects `.gitignore` by default, which may already be enough to skip `target/`/`dist/` without extra configuration.

## Acceptance criteria

- [ ] Editing server source and saving triggers an automatic rebuild + restart of the running server, without manually stopping/restarting it
- [ ] (If in scope) the same for the native client
- [ ] New Makefile target(s) added and documented in `CLAUDE.md`/`README.md` alongside the existing ones
- [ ] `watchexec`'s own installation is documented as a prerequisite, the same way Trunk/`cargo-binutils` already are

## Blocked by

- docs/issues/tooling/makefile-build-targets.md (done - this builds on the Makefile it added)

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/33
