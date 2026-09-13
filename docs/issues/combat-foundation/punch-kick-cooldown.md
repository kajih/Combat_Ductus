## Parent

docs/prd/combat-foundation-hardening.md

## What to build

Add a cooldown to Punch and Kick, mirroring the pattern `combat`'s Special already uses (a tick-based cooldown gate inside `MatchState::apply_attack`), but with a twist requested directly and refined via design review below: Punches land faster than Kicks, rather than "two free punches then a wait."

**Decided mechanic**: one shared "last attack" tick per Character, updated by *either* Punch or Kick. A new Punch requires half of Kick's cooldown duration to have elapsed since that shared timestamp; a new Kick requires the full duration. This means either attack effectively gates the other - throwing a Kick blocks a Punch for half the Kick cooldown too (not just future Kicks), and throwing a Punch blocks a future Kick until the full Kick-cooldown duration has passed since that Punch. There is no special "two free punches" allowance - Punch simply has a shorter, steady cadence than Kick, forever:

```
T = Kick's cooldown duration; Punch's threshold is T/2.
t=0:  Punch  (ok)
t=5:  Punch  (T/2=5 since t=0 - ok, just barely)
t=10: Punch  (T/2=5 since t=5 - ok)
t=10: Kick   (needs T=10 since t=5 - REJECTED, only 5 have passed)
t=15: Kick   (10 have passed since t=5 - ok)
```

This single shared timestamp lives inside `CharacterState` (alongside `special_last_cast_tick`), not as separate server_net.rs state - which means it resets for free whenever `MatchState::new()` runs (a Match restart), with no extra reset code needed, the same way Special's own cooldown already resets for free today.

**Decided: silent rejection, not a whiff.** A cooldown-gated Punch/Kick has no animation and no other feedback at all - unlike today's out-of-range/airborne whiffs (which still play the swing), a cooldown-gated attempt is simply dropped. This was a deliberate simplification: an ideal version would play a distinct "attempted but couldn't" partial animation (an arm/leg motion recognizably *not* a full Punch/Kick), but that's considered too complex for now and left as a future refinement, not part of this issue's scope.

Remaining open branch: the exact tuning values (Kick's cooldown duration, from which Punch's is derived as half) - deferred, placeholder tuning values like every other timing constant already in `combat.rs`.

## Acceptance criteria

- [x] Kick cannot be thrown again until a full cooldown has elapsed since the last Punch *or* Kick, whichever was more recent
- [x] Punch cannot be thrown again until half that cooldown has elapsed since the last Punch *or* Kick, whichever was more recent
- [x] A cooldown-gated Punch/Kick attempt produces no damage and no animation/feedback of any kind
- [x] The cooldown timestamp lives in `CharacterState` and is confirmed to reset naturally on a Match restart (no separate reset code needed, verified by a test)
- [x] Existing Punch/Kick damage, range-check, and airborne-lockout behavior is unchanged - this only adds a new gating condition alongside them
- [x] Verified by unit tests mirroring `combat.rs`'s existing Special-cooldown test (`special_is_gated_by_cooldown`), including the cross-blocking case (a Kick delaying the next Punch, and vice versa), plus a server integration test exercising rapid repeated J/K presses

## Blocked by

None - can start immediately (`combat-rules-module.md` and `punch-kick-health-depletion.md`, both already done, are what this extends).

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/28
