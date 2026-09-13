## Parent

docs/prd/combat-foundation-hardening.md

## What to build

Show each connected client, somewhere on the in-Match screen, which player they're controlling (Player 1 / Player 2) or that they're a spectator - today there's no way for a client to tell. Worth having now that a real second client and spectators both actually exist (`second-client-controls-player-two.md`), not just Player 1 against an Idle Opponent.

This needs a new piece of wire protocol, not just client UI: the server already assigns each connection a player slot (or none, for a spectator - see `second-client-controls-player-two.md`'s `SlotAssignment`), but that assignment is purely internal bookkeeping in `server_net.rs` today and is never communicated back to the connection itself.

**Decided: a one-time message, not a `StateSnapshot` field.** Checked how the broadcast actually works: `step_simulation` serializes one `StateSnapshot` once per tick and every connection's write loop forwards that exact same string - there's no per-connection serialization today. Embedding a per-connection slot in every snapshot would mean restructuring that into per-connection serialization, real added cost for a value that never changes after connecting. Instead, send a one-time message directly on a connection's own stream, right after `assign_slot` runs in `accept_connections` - before it ever starts forwarding the shared broadcast.

**Decided: a formal tagged `ServerMessage` enum**, not an ordering hack. `client_net.rs`'s `poll()` currently assumes every incoming text message deserializes directly as a bare `StateSnapshot`. Rather than relying on "the first message is always special, everything after is a snapshot" (fragile - breaks silently if a message is ever reordered, delayed, or a third message kind is added later), introduce a proper `net_protocol::ServerMessage` enum (e.g. `YourSlot(Option<Player>)` / `Snapshot(StateSnapshot)`, serde-tagged) that both message kinds go through, and update `server_net.rs`'s sends and `client_net.rs`'s parsing accordingly.

## Acceptance criteria

- [x] The server sends each connection a one-time message, right after slot assignment, carrying that connection's own `Option<Player>`
- [x] Both server->client message kinds (the one-time slot message and every ongoing `StateSnapshot`) go through a single, self-describing `ServerMessage` wire type - not positional/ordering-based disambiguation
- [x] The client stores its own role in a resource (mirroring how `LatestSnapshot` already works) and displays a persistent, clearly-visible indicator of it throughout the Match ("Player 1" / "Player 2" / "Spectating")
- [x] Verified by connecting as the 1st, 2nd, and 3rd simultaneous client and confirming each shows the correct role

## Blocked by

None - the spectator state this needs to detect already exists as of `second-client-controls-player-two.md`; `spectator-mode/spectator-connections.md`'s own future spectator-experience work is independent of this.

## Downstream dependency

`character-select/pick-character-on-connect-screen.md` depends on this landing first - its character picker must only show to a connection that actually holds a real player slot, never to a spectator, which requires already knowing the connection's role.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/27
