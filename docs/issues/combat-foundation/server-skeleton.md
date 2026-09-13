## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

A headless `server` binary — a Bevy app using only `MinimalPlugins`, `cfg`-gated out of the wasm build entirely since a browser cannot bind a listening socket. It binds a WebSocket listener, accepts a connection, runs the authoritative `combat` simulation on a fixed tick regardless of whether a client is connected (Player 2 exists as a stationary Idle Opponent from the very start), applies incoming input events from `net_protocol` to Player 1's simulated state, and broadcasts a state snapshot to all connected clients every tick.

## Acceptance criteria

- [ ] The server binary builds only for native targets; it is not part of the wasm client build
- [ ] The server binds a WebSocket listener on a configurable port and accepts at least one client connection
- [ ] The server steps the `combat` simulation every tick regardless of whether a client is connected
- [ ] The server applies a connected client's input events to Player 1's simulated state
- [ ] The server broadcasts a `net_protocol` state snapshot to all connected clients every tick
- [ ] Verified via an automated test or a minimal script/mock WebSocket client connecting and observing snapshots arrive

## Blocked by

- docs/issues/combat-foundation/combat-rules-module.md
- docs/issues/combat-foundation/net-protocol-messages.md

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/5
