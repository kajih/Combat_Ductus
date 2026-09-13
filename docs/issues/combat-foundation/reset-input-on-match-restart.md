## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

Fix a movement bug reported directly: if a player is holding a movement key (A/D) at the moment their attack lands the winning blow, restarting the Match via the restart control leaves that Character already moving on the very first tick of the new Match, even though the key isn't actually held anymore.

Root cause (diagnosed, not just observed): `PlayerInputPlugin`'s input systems (`send_movement_input` etc., in `input.rs`) only run `run_if(in_state(AppState::InMatch))`. The instant the Match ends, the client transitions to `AppState::MatchEnded` and those systems stop running entirely - so if the player releases the key while the Match-Ended screen is showing, the `MoveLeft(false)`/`MoveRight(false)` release event is never sent to the server at all. Server-side `HeldMovement` for that player is therefore stuck at whatever it was the instant the Match ended. `InputEvent::RequestRestart`'s handling in `server_net.rs` resets `MatchState` to a fresh Match but never resets `HeldMovement`, so the stale held-direction immediately starts being applied again from the new Match's very first tick.

This is the same *class* of bug `reset-input-on-disconnect.md` already fixed, but not a shared code path with it - that fix works by the server detecting a TCP connection actually closing (a genuinely separate, per-connection mechanism), and the server has no equivalent "client left `AppState::InMatch`" signal to hook a common fix into. This is a standalone fix: the `RequestRestart` handler should reset `HeldMovement` right alongside `match_state.0 = MatchState::new()`.

**Decided: reset both players' `HeldMovement` unconditionally**, not just whichever player happened to have stale state. `MatchState::new()` already resets both Characters fully (fresh Health, fresh position) regardless of who won or lost - resetting `HeldMovement` for both symmetrically matches that existing "a restart means everyone gets a clean slate" behavior, and is simpler than trying to track which specific player might be affected.

## Acceptance criteria

- [ ] Winning a Match while holding a movement key, then restarting, does not leave that Character moving on its own once the new Match begins
- [ ] This holds even when the key was released while the Match-Ended screen was showing (the release event that never reached the server is accounted for, not relying on lucky timing)
- [ ] Both players' `HeldMovement` is reset on restart, not just the winner's - e.g. the losing player was also holding a key at match-end
- [ ] Existing disconnect-triggered `HeldMovement` reset (`reset-input-on-disconnect.md`) is unaffected
- [ ] Verified by winning while holding A or D, restarting, and confirming the Character stays still until a movement key is freshly pressed again

## Blocked by

None - can start immediately (`restart-match.md` and `reset-input-on-disconnect.md`, both already done, are exactly what this hardens).
