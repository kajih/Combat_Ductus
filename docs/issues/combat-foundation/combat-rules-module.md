## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

A pure Rust `combat` module implementing Health, damage application for Punch (1 damage) and Kick (2 damage), a 1D range check per attack (facing-aware, excludes airborne targets per the Jump definition), Special (Motivational Speech) gating — minimum cast range floor, cooldown, and recent-damage lockout — and Match-end detection (either Character's Health reaches 0). No Bevy `App` dependency: it operates on simple state (positions, Facing, timers) and returns simple results, so it can be exercised directly by `#[test]` functions with no rendering or networking involved.

## Acceptance criteria

- [ ] Punch reduces the opponent's Health by 1, Kick by 2, when the target is within that attack's range, in the attacker's fixed Facing direction, and not airborne
- [ ] Health cannot go below 0 (clamped) and is not damaged further once at 0
- [ ] The range check correctly rejects hits when the opponent is outside range, on the wrong side relative to Facing, or airborne
- [ ] Motivational Speech only succeeds when the opponent is farther than its minimum cast range, its cooldown has elapsed, and no recent-damage lockout is active; when all three hold it deals 1 damage regardless of distance/Facing
- [ ] Match-end is reported exactly when either Character's Health reaches 0, and never before
- [ ] Unit tests cover boundary conditions for each rule above (exactly at range, exactly at the minimum-cast-range floor, exactly at cooldown expiry, exactly at the edge of the lockout window)

## Blocked by

None - can start immediately

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/7
