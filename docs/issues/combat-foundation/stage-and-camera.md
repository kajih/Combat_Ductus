## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

Render the single fixed Stage (the office background art already present in the project) with a camera framed correctly for a 1v1 fighting-game view — a static viewport with no scrolling or following, and correct aspect handling for the web build.

## Acceptance criteria

- [ ] The Stage background renders full-frame with no visible letterboxing/cropping at typical web viewport sizes
- [ ] The camera is static — no follow/scroll behavior, matching the Stage's definition as a single fixed environment
- [ ] Verified by running the app and visually confirming the Stage renders correctly in both the native dev build and the wasm/Trunk build

## Blocked by

None - can start immediately
