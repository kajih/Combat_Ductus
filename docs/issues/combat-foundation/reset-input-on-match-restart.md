## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

Fix a movement bug reported directly: if a player is holding a movement key (A/D) at the moment their attack lands the winning blow, restarting the Match via the restart control leaves that Character already moving on the very first tick of the new Match, even though the key isn't actually held anymore.

Root cause (diagnosed, not just observed): `PlayerInputPlugin`'s input systems (`send_movement_input` etc., in `input.rs`) only run `run_if(in_state(AppState::InMatch))`. The instant the Match ends, the client transitions to `AppState::MatchEnded` and those systems stop running entirely - so if the player releases the key while the Match-Ended screen is showing, the `MoveLeft(false)`/`MoveRight(false)` release event is never sent to the server at all. Server-side `HeldMovement` for that player is therefore stuck at whatever it was the instant the Match ended. `InputEvent::RequestRestart`'s handling in `server_net.rs` resets `MatchState` to a fresh Match but never resets `HeldMovement`, so the stale held-direction immediately starts being applied again from the new Match's very first tick.

This is the same class of bug `reset-input-on-disconnect.md` already fixed for disconnects - the fix here is the same technique (reset `HeldMovement` at the moment of the state transition), just triggered by a restart instead of a disconnect.

## Acceptance criteria

- [ ] Winning a Match while holding a movement key, then restarting, does not leave that Character moving on its own once the new Match begins
- [ ] This holds even when the key was released while the Match-Ended screen was showing (the release event that never reached the server is accounted for, not relying on lucky timing)
- [ ] Existing disconnect-triggered `HeldMovement` reset (`reset-input-on-disconnect.md`) is unaffected
- [ ] Verified by winning while holding A or D, restarting, and confirming the Character stays still until a movement key is freshly pressed again

## Blocked by

None - can start immediately (`restart-match.md` and `reset-input-on-disconnect.md`, both already done, are exactly what this hardens).
