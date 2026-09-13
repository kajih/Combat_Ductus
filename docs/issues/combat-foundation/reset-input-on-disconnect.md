## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

When a client's WebSocket connection is dropped - closed, errored, or otherwise lost, not just a clean disconnect - the server must reset that connection's held-movement state instead of leaving whatever was last pressed applied forever. Verified directly in the current code: nothing clears `HeldMovement` when a connection goes away, so a client that disconnects while holding a movement key leaves that Character walking in that direction indefinitely (until it hits the Stage bound), with no one there to stop it.

## Acceptance criteria

- [x] Disconnecting a client while a movement key was held stops that Character from continuing to move on the very next server tick after the disconnect is detected
- [x] This holds regardless of which direction was held, and even for an abrupt disconnect (network drop, killed process), not just a clean close
- [x] Verified by connecting, holding a movement key, forcibly disconnecting (e.g. killing the client process), and confirming the Character stops moving rather than continuing indefinitely

## Blocked by

None - can start immediately (`server-skeleton.md` and `ground-movement.md`, both already done, are what this hardens).

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/19
