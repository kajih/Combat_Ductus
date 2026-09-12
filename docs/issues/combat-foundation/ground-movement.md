## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

End-to-end horizontal movement: the client captures A/D key input, sends movement input events to the server via `net_protocol`, the server updates Player 1's position within the `combat` simulation each tick, and the resulting position is reflected back to the client and rendered.

## Acceptance criteria

- [ ] Holding A/D moves Player 1's Character left/right on screen, driven by server-simulated position (never predicted or simulated client-side)
- [ ] Movement respects stage bounds (the Character cannot walk off the Stage) — bound values are placeholders, exact tuning deferred
- [ ] Releasing the input stops movement within one server tick
- [ ] Verified by running server + client and visually confirming responsive movement over the WebSocket connection

## Blocked by

- docs/issues/combat-foundation/server-skeleton.md
- docs/issues/combat-foundation/render-characters-from-server-state.md
