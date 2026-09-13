## Parent

docs/prd/combat-foundation-hardening.md

## What to build

Found via design review, not yet covered by any filed issue: `restart-match.md` deliberately made `InputEvent::RequestRestart` player-agnostic - honored "regardless of who sent it" - which made sense when the only alternative to a real player was a stationary Idle Opponent that could never send input anyway. Now that spectators are a real, reachable connection state (`second-client-controls-player-two.md` - a 3rd+ connection that receives every broadcast but controls nothing), that original design decision has a real consequence: a spectator with no stake in the Match can spam-restart it the instant it ends, before the two real players have even finished reading the result.

Restrict `RequestRestart` to connections holding an actual player slot (P1 or P2) - a spectator's restart request should be silently ignored, the same way its movement/attack input already is.

## Acceptance criteria

- [ ] A connection holding the P1 or P2 slot can still restart an ended Match, as today
- [ ] A spectator's `RequestRestart` has no effect - the Match stays ended until a real player restarts it
- [ ] Verified by having a spectator send `RequestRestart` after a Match ends and confirming the Match stays ended until a P1/P2 connection sends it

## Blocked by

- docs/issues/combat-foundation/second-client-controls-player-two.md (done - the per-connection slot tracking this needs to distinguish a player from a spectator)

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/31
