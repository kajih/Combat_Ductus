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

- [x] `KICK_COOLDOWN_TICKS` increased to 250% of its current value (16 -> 40 ticks, or an equivalent increase if the tick rate or other tuning has changed by the time this is picked up)
- [x] Punch's cooldown lands at roughly today's `KICK_COOLDOWN_TICKS` value (~16 ticks) - not still a strict half of the new, larger Kick value, unless that also happens to land close to 16 - **the two constants were decoupled**, so Punch sits at exactly 16 rather than the 20 that keeping the halving would have produced. A compile-time `const _: () = assert!(PUNCH_COOLDOWN_TICKS < KICK_COOLDOWN_TICKS)` now guards the ordering the halving used to guarantee for free
- [x] Existing `combat.rs` unit tests covering the cooldown mechanism still pass - they were written generically against the constants, so none needed logic changes. Two were *renamed*, since their names asserted the dropped 2x relationship: `punch_requires_only_half_the_kick_cooldown_since_the_last_punch_or_kick` -> `punch_requires_its_own_shorter_cooldown_since_the_last_punch_or_kick`, and `a_kick_delays_the_next_punch_by_half_the_kick_cooldown` -> `a_kick_delays_the_next_punch_by_punchs_own_cooldown`
- [ ] Verified by manual playtest against the live server (not unit tests alone) - Punch should feel like today's Kick pace, Kick should feel noticeably slower than that. **Still open**: the code change is tested and lands here, but nobody has played it yet. Worth doing on the same LAN playtest as `verify-connect-screen-default-in-browser.md`

## Finding: both cooldowns now outlast the attack animation

`ATTACK_ANIMATION_TICKS` is 10. With Punch at 16 and Kick at 40, a Character's
swing now always finishes before either attack is usable again - which was
already the stated intent for Kick ("kept longer than `ATTACK_ANIMATION_TICKS`"),
and is now true for Punch as well.

That made one existing test, `a_second_attack_restarts_the_animation_duration`,
unreachable through gameplay: it threw a Kick and then a Punch 9 ticks later,
which the old 8-tick Punch cooldown allowed and the new 16-tick one does not.
The animation-restart behaviour it covers still exists in `start_attack_animation`,
so the test now clears `last_attack_tick` explicitly to keep the animation
restart under test rather than the cooldown gate in front of it.

Worth knowing at the next playtest: attack animations can no longer visibly
overlap or re-trigger mid-swing for a single Character. If that reads as
sluggish rather than weighty, `ATTACK_ANIMATION_TICKS` is the knob, not the
cooldowns.

## Blocked by

- docs/issues/combat-foundation/punch-kick-cooldown.md (done - this refines its tuning values, not its mechanism)

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/38
