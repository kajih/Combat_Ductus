## Parent

docs/prd/combat-foundation-hardening.md

## What to build

Found via direct playtesting/design review, not yet covered by any filed issue: today, a real connected player disconnecting *mid-Match* (not before the Match starts) has no resolution at all - `second-client-controls-player-two.md`'s disconnect handling just resets that slot's held movement and leaves the Character stationary, full Health, taking hits from whoever's left connected with zero resistance. Nothing declares a winner, pauses the Match, or otherwise treats this any differently from the pre-Match Idle Opponent state. `reconnect-match-state-behavior.md`/ADR 0009 covers what a *new or returning* connection sees - this is the other half: what happens to the Match, from the *remaining* player's perspective, the instant their opponent vanishes.

Decided: a real connected player disconnecting while the Match is in progress is a forfeit - the remaining player is immediately declared the winner. This only applies when **both** slots were held by real connected clients at the moment of disconnect; a disconnect while the other slot is still an unclaimed Idle Opponent does nothing special (there's no real opponent to award a win to, and no one connected to see a result screen either) - the Match simply continues exactly as it does today.

This needs a way to tell a forfeit win apart from a normal health-depleted win, since the Match-Ended screen should say the opponent disconnected rather than showing a plain "X wins!" that reads as though they were actually beaten. At minimum this means extending `net_protocol::MatchStatus::Ended` with some notion of *how* the Match ended (e.g. a reason/cause alongside `winner`), not just who won.

## Acceptance criteria

- [x] When both player slots are held by real connected clients and one disconnects while the Match is in progress, the Match immediately ends with the remaining player declared the winner
- [x] A disconnect while the other slot is still an unclaimed Idle Opponent does not end the Match or declare a winner - the remaining player's Match continues exactly as today
- [x] The Match-Ended screen distinguishes a forfeit win from a normal health-depleted win, indicating the opponent disconnected
- [x] A disconnect after the Match has already ended has no additional effect (mirrors `restart-match.md`'s existing "only meaningful while in progress" pattern) - `MatchState::forfeit` respects the same `has_ended()` guard every other state-mutating method does
- [x] Verified by disconnecting one of two real connected clients mid-Match and confirming the remaining client immediately sees a forfeit-win screen naming the disconnect - covered by an automated `server_net` integration test over real WebSocket connections (`a_real_players_mid_match_disconnect_forfeits_the_match_to_the_remaining_player`), not manually eyeballed

**Implementation note:** detecting a disconnect is never instant (it takes a failed broadcast send on the *next* tick to notice at all), which opened a narrow race - an unrelated connection briefly claiming the *other*, already-vacant slot in that detection window could read identically to "the opponent was real and connected." Fixed with a short occupancy-settle debounce (`server_net::MIN_OPPONENT_SETTLE`, 75ms) - a slot only counts as a real opponent for forfeit purposes once it's been continuously taken for at least that long. Real players never reconnect within a fraction of a tick of each other, so this only ever debounces impossible-for-a-human timing, not genuine gameplay; discovered via `a_freed_player_slot_is_reassigned_to_the_next_connection` and `a_connection_taking_over_a_vacated_slot_inherits_its_current_state_not_a_fresh_start` (renamed to `a_connection_taking_over_a_forfeited_slot_inherits_the_ended_match_not_a_fresh_start`, since its own scenario - P2 disconnecting from an ongoing real-vs-real Match - is now exactly what forfeits it) going from reliably passing to reliably failing once forfeit landed.

## Blocked by

- docs/issues/combat-foundation/second-client-controls-player-two.md (done - real per-slot connection tracking this needs)

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/30
