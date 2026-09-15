## What to build

Let a 3rd (or later) WebSocket connection to the server observe an in-progress Match without controlling anything.

This needs real groundwork, not just a rendering tweak: the server currently has no concept of connection identity at all - it applies every connected client's input unconditionally to Player 1, because only one client has ever been supported up to this point. Spectator support requires the server to assign each connection a role as it connects (P1, P2, or spectator) and only apply input from whichever connection actually controls a player. The broadcast channel that streams state snapshots is already multi-subscriber by construction (noted since `server-skeleton.md`), so getting a spectator's *view* is close to free once role assignment exists - the real work is the role assignment itself.

Note: this is a natural companion to real two-client play (a second real client actually connecting and controlling Player 2), which itself isn't a tracked issue yet as of this writing - today Player 2 is only ever the stationary Idle Opponent. Spectator mode and real two-client play likely want the same connection-identity groundwork; worth sequencing or designing together rather than assuming this is fully self-contained.

Not relevant yet as of this writing - filed for later, not blocking any current work.

## Acceptance criteria

- [x] The server assigns each connection a role (P1, P2, or spectator) as it connects, rather than blindly applying all input to Player 1 - `server_net::accept` assigns the first free slot and falls back to spectator (`slot: None`), and tells the connection which it got via the one-time `YourSlot` message (`each_connection_is_told_its_own_slot_right_after_connecting`)
- [x] A spectator connection's input, if it sends any, has no effect on the Match - movement/attacks are dropped in `server_net::simulation` for a connection with no slot (`a_third_connection_is_a_spectator_and_controls_neither_player`), and `RequestRestart` is silently ignored the same way (`a_spectators_restart_request_has_no_effect_but_a_players_still_works`, added later by `restrict-restart-to-players.md`)
- [x] A spectator's client receives the same state snapshots as a playing client and renders the same live Match - the broadcast channel is multi-subscriber and slot-agnostic, and no client-side renderer (`match_characters`, `health_hud`, `stage`) gates on slot; `role_indicator` shows "Spectating" so the role is legible on screen
- [x] Verified by connecting three clients simultaneously - two controlling players, one spectating - and confirming the spectator observes without affecting the Match - by the integration test above, which opens three real WebSocket connections against a real local server and asserts across ten ticks that the spectator's input moves neither Character. Not re-done as a manual three-browser playtest; the test covers the same scenario at the protocol level.

## Blocked by

None - can start immediately, though see the note above about sequencing this alongside real two-client play.

## Outcome

Closed as already-satisfied rather than built. The sequencing note above turned out to be exactly right: the connection-identity groundwork this issue described as "the real work" was built by `second-client-controls-player-two.md` (#20) and `connection-identity-indicator.md` (#27), and the spectator behaviour came with it, as this issue predicted it would.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/32
