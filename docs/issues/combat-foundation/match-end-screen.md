## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

When `combat` reports a Character's Health has reached 0, the server marks the Match ended (recording which Character won) and broadcasts this terminal state; the client transitions to a static Match-Ended screen showing the winner, with no rematch/restart flow.

## Acceptance criteria

- [ ] The server stops applying further damage/movement updates once Match-end is detected
- [ ] The server broadcasts which Character won as part of (or immediately following) the snapshot in which Health reaches 0
- [ ] The client transitions from In-Match to a static "X wins" screen on receiving the Match-ended state
- [ ] No rematch/restart control exists on this screen — refreshing the page is the only way to start over, per the PRD's scope
- [ ] Verified by depleting a Character's Health to 0 during a run and confirming the screen appears with the correct winner

## Blocked by

- docs/issues/combat-foundation/punch-kick-health-depletion.md
