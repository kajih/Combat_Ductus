## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

Add a cooldown to Punch and Kick, mirroring the pattern `combat`'s Special already uses (a tick-based cooldown gate inside `MatchState::apply_attack`), but with a twist requested directly: Kick is gated by a cooldown after every single Kick, while Punch allows two consecutive Punches (one per arm) before its own cooldown kicks in on the third.

Concretely, at minimum this should settle:
- Whether Punch and Kick track cooldown independently of each other (this seems right - they're separate limbs/animations, and the request already treats them differently) or share state in some way
- The exact mechanic for Punch's "2 before cooldown" - e.g. a rolling counter of consecutive Punches that resets once a full cooldown elapses since the last one, vs. some other bookkeeping
- Whether a Punch/Kick thrown during its own cooldown is rejected outright with no feedback at all (like a gated Special) or still plays its whiff animation (like today's out-of-range whiff) while simply failing to land
- Exact tuning values (cooldown durations for each) - deferred, placeholder tuning values like every other timing constant already in `combat.rs`

## Acceptance criteria

- [ ] Kick cannot be thrown again immediately after landing or whiffing - a cooldown applies after every Kick
- [ ] Punch can be thrown twice in a row (two arms) before a cooldown gates the third attempt
- [ ] Punch and Kick cooldowns are independent - being on Kick cooldown doesn't block Punch, and vice versa
- [ ] Existing Punch/Kick damage, range-check, and airborne-lockout behavior is unchanged - this only adds a new gating condition alongside them
- [ ] Verified by unit tests mirroring `combat.rs`'s existing Special-cooldown test (`special_is_gated_by_cooldown`), plus a server integration test exercising rapid repeated J/K presses

## Blocked by

None - can start immediately (`combat-rules-module.md` and `punch-kick-health-depletion.md`, both already done, are what this extends).
