## What to build

Add sound effects for core combat feedback - at minimum, a landed Punch, a landed Kick, and the Match ending - and whatever else naturally falls out of implementing those (e.g. a distinct whiff sound to match the arm/leg swing that already plays on a miss, per `punch-kick-health-depletion.md`).

This was explicitly deferred during `punch-kick-health-depletion.md`'s design ("audio is text-only for v1... defer all other audio to a later polish milestone"). Real playtesting of that issue showed missing *visual* feedback mattered more than expected once actually played (the limb-swing animation had to be added after the fact for the same reason); audio feedback is worth tracking deliberately rather than assuming it's low-priority polish.

Bevy's audio stack (`bevy_audio`, `rodio`/`cpal`) is already part of the dependency tree and initializes today (confirmed in the wasm console log), just entirely unused - the capability already exists, this is about sourcing/creating actual sound assets and wiring playback to the relevant combat events. Unlike the placeholder art, no placeholder sound assets exist in the repo yet - sourcing or generating some is part of this work, not a separate prerequisite.

## Acceptance criteria

- [ ] Landing a Punch plays a distinct sound
- [ ] Landing a Kick plays a distinct sound
- [ ] The Match ending plays a sound
- [ ] Verified by playing a Match and confirming each of the above triggers the expected sound at the expected moment

## Blocked by

None - can start immediately (`punch-kick-health-depletion.md`, already done, provides everything needed for the Punch/Kick triggers; the Match-end trigger only needs the existing `MatchStatus::Ended` snapshot field, not the not-yet-built `match-end-screen.md`).
