## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

Wire the connected client to render both Characters via `character_rig`, driven by the server's state snapshots rather than any local placeholder. Player 1 reflects the connected client's Character; Player 2 renders as the stationary Idle Opponent per the server's simulated state.

## Acceptance criteria

- [ ] On entering In-Match state, both Player 1 and Player 2 Characters spawn via `character_rig` with correct fixed Facing (Player 1 faces right, Player 2 faces left, per `CONTEXT.md`)
- [ ] Each Character's on-screen position updates from incoming server state snapshots rather than being hardcoded client-side
- [ ] Player 2 renders as visibly stationary/idle (Idle Opponent), since no second client is connected in this milestone
- [ ] Verified by running the server and one client and visually confirming both Characters appear correctly on the Stage

## Blocked by

- docs/issues/combat-foundation/client-connect-and-lifecycle.md
- docs/issues/combat-foundation/idle-character-on-stage.md
