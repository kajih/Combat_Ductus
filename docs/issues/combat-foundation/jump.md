## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

End-to-end Jump (Space): vertical repositioning for Player 1's Character, purely movement, with Punch/Kick disabled while airborne per the existing Jump definition and `combat`'s airborne rule.

## Acceptance criteria

- [ ] Pressing Space moves Player 1's Character upward and back down (a jump arc or simple rise/fall), driven by server-simulated state
- [ ] Punch/Kick inputs are ignored (produce no effect) while the Character is airborne
- [ ] Landing restores the ability to Punch/Kick
- [ ] Verified by running server + client and confirming both the jump movement and the attack-lockout-while-airborne behavior

## Blocked by

- docs/issues/combat-foundation/combat-rules-module.md
- docs/issues/combat-foundation/ground-movement.md
