## Parent

None - code-health/maintainability, not a milestone feature.

## What to build

`server_net.rs` has grown to roughly 1600 lines, of which roughly 1160 are its `#[cfg(test)]` module - a thorough integration test suite (the whole accept-loop/slot-assignment/simulation stack, driven against a real local server over real WebSocket connections), but unwieldy sitting inline in one file alongside ~460 lines of actual production code. Split the tests out so the production code and its test suite are easier to navigate independently.

Remaining open branch: whether to move the existing `mod tests { ... }` into its own file via `#[path = "..."]` (simplest, keeps everything as unit tests with access to private items), or convert it into a proper `tests/` directory integration-test crate (would need `server_net`'s test-only helpers - `spawn_network_thread`, `run_bevy_app`, etc. - to already be `pub`, which they may or may not already be) - left to whoever implements this to pick based on what's actually least disruptive once they're in there.

## Acceptance criteria

- [ ] `server_net.rs`'s test module lives in its own file, not inline in the ~1600-line file
- [ ] All existing tests still pass, unchanged in behavior and assertions
- [ ] No change to production code behavior - this is a pure file-organization change

## Blocked by

None - can start immediately.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/44
