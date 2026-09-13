## Parent

docs/prd/combat-foundation-hardening.md

## What to build

Retune the Punch/Kick cooldown constants in `combat.rs` after manual playtesting against the live server (see `punch-kick-cooldown.md` for the mechanism itself, which stays unchanged - this is a tuning-only follow-up). Direct feedback from playtesting:

- Today's `KICK_COOLDOWN_TICKS` (16 ticks, ~0.53s at the server's ~30Hz tick rate) actually reads as the right cadence for **Punch**, not Kick.
- Today's derived `PUNCH_COOLDOWN_TICKS` (`KICK_COOLDOWN_TICKS / 2` = 8 ticks, ~0.27s) throws far too fast.
- Kick itself needs to become slower still, on top of that - requested directly as **250% of its current speed** (16 ticks -> 40 ticks).

**Decided direction:**
- Punch's cooldown should land at roughly today's Kick value (~16 ticks).
- Kick's cooldown should become 250% of its current value (16 -> 40 ticks).
- Whether the existing `PUNCH_COOLDOWN_TICKS = KICK_COOLDOWN_TICKS / 2` halving relationship is kept as-is (which would put Punch at 20 ticks once Kick becomes 40 - close to, but not exactly, today's Kick value) or the two constants are decoupled so Punch can sit exactly at ~16 while Kick sits at 40 (a ~2.5x ratio instead of a fixed 2x) is left to whoever implements this. Either is a small, localized change in `combat.rs` - no change to the gating mechanism/logic itself either way.

## Acceptance criteria

- [ ] `KICK_COOLDOWN_TICKS` increased to 250% of its current value (16 -> 40 ticks, or an equivalent increase if the tick rate or other tuning has changed by the time this is picked up)
- [ ] Punch's cooldown lands at roughly today's `KICK_COOLDOWN_TICKS` value (~16 ticks) - not still a strict half of the new, larger Kick value, unless that also happens to land close to 16
- [ ] Existing `combat.rs` unit tests covering the cooldown mechanism (`kick_requires_a_full_cooldown_since_the_last_punch_or_kick`, `punch_requires_only_half_the_kick_cooldown_since_the_last_punch_or_kick`, `a_kick_delays_the_next_punch_by_half_the_kick_cooldown`, `cooldown_gated_attack_starts_no_animation_and_deals_no_damage`) still pass - they're written generically against the constants rather than hardcoded tick counts, so most should keep passing unchanged; update any that assume the exact 2x Punch/Kick ratio if that relationship is dropped
- [ ] Verified by manual playtest against the live server (not unit tests alone) - Punch should feel like today's Kick pace, Kick should feel noticeably slower than that

## Blocked by

- docs/issues/combat-foundation/punch-kick-cooldown.md (done - this refines its tuning values, not its mechanism)

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/38
