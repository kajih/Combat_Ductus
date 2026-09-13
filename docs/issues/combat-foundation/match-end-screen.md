## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

When `combat` reports a Character's Health has reached 0, the server marks the Match ended (recording which Character won) and broadcasts this terminal state; the client transitions to a static Match-Ended screen showing the winner.

Note: the PRD originally scoped this screen with no rematch/restart control (refresh-to-restart accepted). Real playtesting of `punch-kick-health-depletion` showed that gap is more disruptive than expected, so a restart control is now planned — see `docs/issues/combat-foundation/restart-match.md`, which builds on top of this screen rather than folding into it, so this issue's own scope stays just the static end-state screen itself.

## Acceptance criteria

- [ ] The server stops applying further damage/movement updates once Match-end is detected
- [ ] The server broadcasts which Character won as part of (or immediately following) the snapshot in which Health reaches 0
- [ ] The client transitions from In-Match to a static "X wins" screen on receiving the Match-ended state
- [ ] Verified by depleting a Character's Health to 0 during a run and confirming the screen appears with the correct winner

## Blocked by

- docs/issues/combat-foundation/punch-kick-health-depletion.md

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/17
