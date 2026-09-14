## Parent

None - dependency hygiene.

## What to build

The client's `ewebsock` v0.8.0 dependency pulls in `tungstenite` v0.24.0 for its own native WebSocket implementation, while the project's own server-side `tokio-tungstenite` v0.30.0 dependency pulls a separate, newer `tungstenite` v0.30.0 - two different major versions of the same underlying crate coexist in the dependency tree, along with several of its own transitive dependencies duplicated in both versions (`block-buffer`, `digest`, `sha1`, and others - found via `cargo tree --duplicates`). No known functional impact - just extra compile time and binary size.

Check whether a newer `ewebsock` release has moved to a newer `tungstenite`, and upgrade if so and if it doesn't require API changes in `client_net.rs`. If `ewebsock` is genuinely stuck on the old version, there may be nothing to do here beyond documenting it as a known, currently-unfixable duplication rather than leaving it silently rediscoverable via `cargo tree` each time.

## Acceptance criteria

- [ ] Confirmed whether a newer `ewebsock` version resolves the `tungstenite` duplication
- [ ] If an upgrade is available and doesn't require API changes in `client_net.rs`, it's applied and `cargo tree --duplicates` shows the duplication resolved
- [ ] If no upgrade is available, this is documented as a known, currently-unfixable duplication (e.g. a comment near the `ewebsock` dependency in `Cargo.toml`) rather than left silently rediscoverable

## Blocked by

None - can start immediately.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/46
