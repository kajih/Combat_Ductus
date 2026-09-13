## What to build

Let a 3rd (or later) WebSocket connection to the server observe an in-progress Match without controlling anything.

This needs real groundwork, not just a rendering tweak: the server currently has no concept of connection identity at all - it applies every connected client's input unconditionally to Player 1, because only one client has ever been supported up to this point. Spectator support requires the server to assign each connection a role as it connects (P1, P2, or spectator) and only apply input from whichever connection actually controls a player. The broadcast channel that streams state snapshots is already multi-subscriber by construction (noted since `server-skeleton.md`), so getting a spectator's *view* is close to free once role assignment exists - the real work is the role assignment itself.

Note: this is a natural companion to real two-client play (a second real client actually connecting and controlling Player 2), which itself isn't a tracked issue yet as of this writing - today Player 2 is only ever the stationary Idle Opponent. Spectator mode and real two-client play likely want the same connection-identity groundwork; worth sequencing or designing together rather than assuming this is fully self-contained.

Not relevant yet as of this writing - filed for later, not blocking any current work.

## Acceptance criteria

- [ ] The server assigns each connection a role (P1, P2, or spectator) as it connects, rather than blindly applying all input to Player 1
- [ ] A spectator connection's input, if it sends any, has no effect on the Match
- [ ] A spectator's client receives the same state snapshots as a playing client and renders the same live Match
- [ ] Verified by connecting three clients simultaneously - two controlling players, one spectating - and confirming the spectator observes without affecting the Match

## Blocked by

None - can start immediately, though see the note above about sequencing this alongside real two-client play.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/32
