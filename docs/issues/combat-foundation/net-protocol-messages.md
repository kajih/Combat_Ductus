## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

A shared `net_protocol` module defining the wire message types exchanged over the WebSocket connection: client→server input events (movement direction, jump, punch, kick, special) and server→client state snapshots (per-Character position, Facing, Health, airborne state, and Match status). Plain serde-serializable structs/enums, JSON-encoded, with no I/O of their own, so the module compiles unchanged on both the native target and `wasm32-unknown-unknown`.

## Acceptance criteria

- [ ] Input-event and state-snapshot types are defined and derive `Serialize`/`Deserialize`
- [ ] A round-trip serialize→deserialize test exists for each message type and produces an equal value
- [ ] The module compiles for both the native target and `wasm32-unknown-unknown` with no platform-specific code
- [ ] Message shapes carry enough information for the server to apply `combat`'s rules and for the client to render a full frame of state (position, Facing, Health, airborne, Match status) without needing any additional out-of-band data

## Blocked by

None - can start immediately

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/6
