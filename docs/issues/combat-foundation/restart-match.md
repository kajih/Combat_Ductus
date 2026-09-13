## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

A restart control on the Match-Ended screen that starts a fresh Match without requiring anyone to refresh the page or reconnect. This reverses the PRD's original "no rematch/restart flow, refresh-to-restart accepted" scope call — real playtesting of `punch-kick-health-depletion` showed that ending a Match with no way to continue is a real gap, not an acceptable simplification for this stage of the project.

Pressing the control sends a request to the server; the server resets its `combat::MatchState` back to a fresh starting Match (full Health, starting positions, no winner) and broadcasts that it's in progress again. The client transitions from the Match-Ended screen back to the ordinary In-Match view the moment it sees the Match is in progress again, the same way it already reacts to any other state snapshot. No new concept of "lobbies" or multiple matches is needed — there is only ever one Match on the server at a time in this milestone, and restarting it just means resetting that one piece of state.

## Acceptance criteria

- [ ] The Match-Ended screen has a visible, clickable restart control
- [ ] Activating it resets the server's Match to a fresh starting state (full Health for both Characters, starting positions, Match in progress again)
- [ ] The client leaves the Match-Ended screen and returns to the ordinary In-Match view once the server confirms the Match is in progress again
- [ ] A restart request is ignored (or otherwise has no effect) if the Match hasn't actually ended yet - this only resets a concluded Match, not an active one
- [ ] Verified by playing a Match to completion, restarting, and confirming a second Match can be played through to completion the same way

## Blocked by

- docs/issues/combat-foundation/match-end-screen.md

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/16
