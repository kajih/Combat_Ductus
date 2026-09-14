## Parent

None - documentation accuracy, surfaced while setting this project up on a second machine.

## Priority

Low. Nothing is broken and no build is affected; this is stale wording that costs a little confusion, not time. Fold it into the next `CLAUDE.md` edit rather than making a trip for it.

## What to build

`CLAUDE.md` was written when this project lived on one Windows machine, and several passages still say "this machine" as though there is only one. It now also lives on a Linux laptop, and at least one of those statements is outright wrong there:

- **"`gh` (GitHub CLI) is authenticated on this machine"** - was not true on the Linux machine until it was installed mid-session. A session that trusts this line plans around `gh` being available and only finds out when a PR fails to open.
- **"`make` isn't installed by default on Windows - this machine has it via `choco install make`"** - accurate as history, but reads as though Windows is the only environment.
- The `trunk build --release` / `wasm-opt` failure is recorded as Windows-specific. That's still correct, and worth keeping scoped to Windows, but it should be legible as "this happens on Windows" rather than "this happens here".

The fix is wording, not restructuring: make each environment-specific claim say *which* environment it applies to. The existing `.cargo/config.toml` guidance is already a good model - it names both platforms explicitly and lets the reader pick.

Worth deciding once, rather than per-sentence: whether `CLAUDE.md` should name the two machines as such (a Windows desktop and a Linux laptop), or drop "this machine" entirely in favour of naming platforms. The latter ages better if a third environment ever appears.

## Acceptance criteria

- [ ] No statement in `CLAUDE.md` claims a tool is installed/authenticated "on this machine" without saying which platform or environment it holds for
- [ ] The `gh` line in particular reflects that it needs installing per machine (`sudo pacman -S github-cli && gh auth login` on Arch), rather than asserting it's already authenticated
- [ ] Genuinely Windows-specific entries (the `wasm-opt` failure) stay scoped to Windows and remain easy to find
- [ ] `README.md` is checked for the same pattern, since it describes prerequisites for anyone cloning the repo

## Blocked by

Nothing.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/56
