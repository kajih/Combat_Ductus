## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

End-to-end Motivational Speech: pressing H sends a Special input event; the server checks `combat`'s gating (minimum cast range exceeded, cooldown elapsed, no recent-damage lockout) and, if allowed, applies 1 damage regardless of distance/Facing; the client plays the speech-bubble VFX showing that Character's own placeholder text line.

## Acceptance criteria

- [x] Pressing H when gating conditions are met deals 1 damage to the opponent regardless of distance or Facing
- [x] Pressing H when the opponent is within the minimum cast range, or during cooldown, or during the recent-damage lockout window, has no effect
- [x] The speech-bubble VFX displays on a successful cast, showing that Character's specific placeholder text line
- [x] Successive casts respect the cooldown (cannot be spammed back-to-back)
- [ ] Verified by running server + client and exercising a successful cast plus each gating-rejection case

## Blocked by

- docs/issues/combat-foundation/combat-rules-module.md
- docs/issues/combat-foundation/punch-kick-health-depletion.md
