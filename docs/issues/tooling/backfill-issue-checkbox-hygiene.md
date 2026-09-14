## Parent

None - documentation hygiene, not a code change.

## What to build

A number of already-closed, already-implemented issues under `docs/issues/` still show unchecked acceptance-criteria boxes - they predate the more rigorous "check off what's actually verified against the running app" habit established in later sessions. Confirmed examples (all `CLOSED` on GitHub, all genuinely implemented and working): `combat-rules-module.md`, `ground-movement.md`, `server-skeleton.md` - there are more across the `docs/issues/` tree with the same pattern.

Go through every issue file whose GitHub issue is `CLOSED` and reconcile its acceptance-criteria checkboxes against the current codebase, so the local files can be trusted at a glance to reflect actual completion state - which is the whole point of them being the source of truth (per `CLAUDE.md`'s git workflow section).

## Acceptance criteria

- [ ] Every issue file whose GitHub issue is closed has all its genuinely-satisfied acceptance criteria checked off
- [ ] Any criterion that's still legitimately unverified (e.g. a specific manual multi-client scenario never actually performed) stays unchecked, with a short note why - not checked off just to tidy up
- [ ] No file's checkboxes are changed without actually confirming the behavior against the current code, not just assumed from the issue being closed
- [ ] Closed issues that turn out to be *not* fully satisfied by the current code (if any) are flagged back to the user rather than silently checked off

## Blocked by

None - can start immediately.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/43
