## Parent

docs/prd/combat-foundation-hardening.md

## What to build

Found via direct playtesting/design review, not yet covered by any filed issue: today, a real connected player disconnecting *mid-Match* (not before the Match starts) has no resolution at all - `second-client-controls-player-two.md`'s disconnect handling just resets that slot's held movement and leaves the Character stationary, full Health, taking hits from whoever's left connected with zero resistance. Nothing declares a winner, pauses the Match, or otherwise treats this any differently from the pre-Match Idle Opponent state. `reconnect-match-state-behavior.md`/ADR 0009 covers what a *new or returning* connection sees - this is the other half: what happens to the Match, from the *remaining* player's perspective, the instant their opponent vanishes.

Decided: a real connected player disconnecting while the Match is in progress is a forfeit - the remaining player is immediately declared the winner. This only applies when **both** slots were held by real connected clients at the moment of disconnect; a disconnect while the other slot is still an unclaimed Idle Opponent does nothing special (there's no real opponent to award a win to, and no one connected to see a result screen either) - the Match simply continues exactly as it does today.

This needs a way to tell a forfeit win apart from a normal health-depleted win, since the Match-Ended screen should say the opponent disconnected rather than showing a plain "X wins!" that reads as though they were actually beaten. At minimum this means extending `net_protocol::MatchStatus::Ended` with some notion of *how* the Match ended (e.g. a reason/cause alongside `winner`), not just who won.

## Acceptance criteria

- [ ] When both player slots are held by real connected clients and one disconnects while the Match is in progress, the Match immediately ends with the remaining player declared the winner
- [ ] A disconnect while the other slot is still an unclaimed Idle Opponent does not end the Match or declare a winner - the remaining player's Match continues exactly as today
- [ ] The Match-Ended screen distinguishes a forfeit win from a normal health-depleted win, indicating the opponent disconnected
- [ ] A disconnect after the Match has already ended has no additional effect (mirrors `restart-match.md`'s existing "only meaningful while in progress" pattern)
- [ ] Verified by disconnecting one of two real connected clients mid-Match and confirming the remaining client immediately sees a forfeit-win screen naming the disconnect

## Blocked by

- docs/issues/combat-foundation/second-client-controls-player-two.md (done - real per-slot connection tracking this needs)

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/30
