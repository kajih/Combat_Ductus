## Parent

None - code-health/maintainability, not a milestone feature.

## What to build

`server_net.rs` has grown to roughly 1600 lines, of which roughly 1160 are its `#[cfg(test)]` module - a thorough integration test suite (the whole accept-loop/slot-assignment/simulation stack, driven against a real local server over real WebSocket connections), but unwieldy sitting inline in one file alongside ~460 lines of actual production code. Split the tests out so the production code and its test suite are easier to navigate independently.

Remaining open branch: whether to move the existing `mod tests { ... }` into its own file via `#[path = "..."]` (simplest, keeps everything as unit tests with access to private items), or convert it into a proper `tests/` directory integration-test crate (would need `server_net`'s test-only helpers - `spawn_network_thread`, `run_bevy_app`, etc. - to already be `pub`, which they may or may not already be) - left to whoever implements this to pick based on what's actually least disruptive once they're in there.

**Resolution:** neither of the above - `server_net` became a folder module instead (`server_net.rs` as the slim root plus `server_net/accept.rs`, `server_net/simulation.rs`, `server_net/tests.rs`), so the production code itself got split along its two real halves (the network accept loop vs. the Bevy simulation) at the same time as the tests moved out, rather than splitting only the tests and leaving a still-large single production file behind. Everything stays under `crate::server_net::*` - `accept`/`simulation` aren't `pub`, only `spawn_network_thread`/`run_bevy_app` are re-exported at the root, so external code sees no change at all. `tests.rs` keeps unit-test-style access to private items via `use super::*`, same as the original inline module - no test-only items needed to become `pub`.

Scope also grew to cover `combat.rs` (503 production lines / 554 test lines - the same lopsided shape as `server_net.rs`) in the same pass, per direct user request once this issue was picked up - it became `combat.rs` (production code, unchanged in kind - still one deep module, not further split) + `combat/tests.rs`. `docs/issues/tooling/backfill-issue-checkbox-hygiene.md`-style scope creep, but decided live with the user rather than silently assumed.

`combat/tests.rs` itself then grew one folder-module level deeper, again per direct user request: it's now the slim root (the shared `state_at` helper plus `mod` declarations) over `combat/tests/combat.rs` (Punch/Kick damage, range, animation, cooldown, Match-end, forfeit - 20 tests), `combat/tests/movement.rs` (ground movement and Jump - 9 tests), and `combat/tests/special.rs` (Motivational Speech gating and speech-bubble VFX - 8 tests) - split by what's under test, not by any API grouping in `combat.rs` itself. All still `crate::combat::tests::*`, not separate top-level modules, same principle as `server_net`'s split.

## Acceptance criteria

- [x] `server_net.rs`'s test module lives in its own file, not inline in the ~1600-line file - and its production code is now split into `accept.rs`/`simulation.rs` too, per the resolution above
- [x] All existing tests still pass, unchanged in behavior and assertions - `cargo test --features dev` 68/68, re-run repeatedly to rule out flakiness from the split itself
- [x] No change to production code behavior - this is a pure file-organization change (verified by `cargo check --all-targets`, `cargo clippy`, `cargo check --target wasm32-unknown-unknown`, and `cargo fmt` all clean, alongside the full test pass)
- [x] (scope addition) `combat.rs`'s test module lives in its own file (further split into `combat/tests/{combat,movement,special}.rs` by test subject), same verification as above

## Blocked by

None - can start immediately.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/44
