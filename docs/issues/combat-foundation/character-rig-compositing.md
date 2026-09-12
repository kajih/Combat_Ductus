## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

The `character_rig` compositing logic per ADR 0007 — a Character assembled at runtime from separate torso, arm, and leg sprite entities, attached to the torso at socket offsets keyed by (Body Type, Facing), for one Body Type to start. Includes the face-photo slot on the torso with its horizontal squash transform per ADR 0006, fed a placeholder photo.

## Acceptance criteria

- [ ] Torso, arm, and leg render as separate entities positioned correctly relative to each other for at least one Body Type and both Facing values
- [ ] The face-photo slot displays a placeholder image with the squash/skew transform applied per Facing
- [ ] Swapping the idle arm/leg sprites for their punch/kick counterparts does not require moving or re-anchoring the torso
- [ ] Z-order between arm and torso matches the attack being displayed (e.g. arm drawn in front of torso on Punch), per ADR 0007
- [ ] Verified by running the app and visually confirming a correctly-assembled, correctly-proportioned Character

## Blocked by

None - can start immediately
