## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

End-to-end Punch (J) and Kick (K): the client sends the attack input event, the server applies `combat`'s range check and damage against Player 2's (Idle Opponent's) Health, broadcasts the updated Health, and the client displays it via a minimal visible Health readout (plain text/number is sufficient — no HUD polish required).

## Acceptance criteria

- [ ] Pressing J when Player 2 is within Punch range reduces Player 2's Health by 1, reflected in both server state and the client display
- [ ] Pressing K when Player 2 is within Kick range reduces Player 2's Health by 2
- [ ] Attacks thrown outside range apply no damage
- [ ] A visible Health readout updates for both Characters as damage is applied
- [ ] Verified by running server + client and both landing and whiffing attacks against the Idle Opponent

## Blocked by

- docs/issues/combat-foundation/combat-rules-module.md
- docs/issues/combat-foundation/ground-movement.md
