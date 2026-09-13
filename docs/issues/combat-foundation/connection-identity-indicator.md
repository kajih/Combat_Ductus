## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

Show each connected client, somewhere on the in-Match screen, which player they're controlling (Player 1 / Player 2) or that they're a spectator - today there's no way for a client to tell. Worth having now that a real second client and spectators both actually exist (`second-client-controls-player-two.md`), not just Player 1 against an Idle Opponent.

This needs a new piece of wire protocol, not just client UI: the server already assigns each connection a player slot (or none, for a spectator - see `second-client-controls-player-two.md`'s `SlotAssignment`), but that assignment is purely internal bookkeeping in `server_net.rs` today and is never communicated back to the connection itself. At minimum this needs a new server -> client message carrying that connection's own `Option<Player>` (mirroring the type `server_net.rs` already uses internally for this) - either a one-time message sent right after a connection is accepted, or a field added to every `StateSnapshot` - which the client then renders as a small, persistent label ("You are Player 1" / "You are Player 2" / "Spectating").

## Acceptance criteria

- [ ] The server communicates each connection's assigned slot (or spectator status) back to that specific connection, not just via the shared broadcast state
- [ ] The client displays a persistent, clearly-visible indicator of its own role throughout the Match
- [ ] Verified by connecting as the 1st, 2nd, and 3rd simultaneous client and confirming each shows the correct role (Player 1 / Player 2 / Spectating)

## Blocked by

None - the spectator state this needs to detect already exists as of `second-client-controls-player-two.md`; `spectator-mode/spectator-connections.md`'s own future spectator-experience work is independent of this.
