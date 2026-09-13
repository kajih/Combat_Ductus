## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

Decide, document, and implement a deliberate rule for what a new or returning connection sees and controls when it joins while a Match is already in progress or has already ended - today there is no rule at all: because only one global `MatchState` exists, a connecting client silently inherits whatever that state already is (possibly mid-fight with reduced Health, or already over), which may surprise players expecting a fresh start.

This is more meaningful once real two-client play exists (`second-client-controls-player-two.md`) - the interesting questions (does a new connection take over an already-occupied player slot? get refused? queue?) only really arise once both player slots can be genuinely occupied. It also overlaps with `restart-match.md` for the "Match already ended" case specifically - worth checking that issue's eventual behavior doesn't already answer part of this before duplicating a decision.

Concretely, at minimum this should settle:
- What happens when a connection arrives and both player slots are already taken (spectate? refuse? something else - `spectator-mode/spectator-connections.md` may already answer this once it exists).
- What happens when a connection arrives after the Match has already ended (show the Match-Ended state immediately? something else?).
- Whether a disconnected player's slot stays "reserved" for them to resume, or is immediately available to whoever connects next.

## Acceptance criteria

- [x] A documented, deliberate rule exists for what a new/returning connection sees and controls when joining a Match already in progress
- [x] A documented, deliberate rule exists for what happens when a new/returning connection joins after a Match has already ended
- [x] The server's actual behavior matches whichever rule is chosen, rather than the current unspecified/accidental behavior
- [ ] Verified by connecting a client mid-Match and again after a Match has ended, and confirming the behavior matches the documented rule

## Blocked by

- docs/issues/combat-foundation/second-client-controls-player-two.md

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/21
