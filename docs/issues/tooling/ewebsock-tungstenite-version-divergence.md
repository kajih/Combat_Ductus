## Parent

None - dependency hygiene.

## What to build

The client's `ewebsock` v0.8.0 dependency pulls in `tungstenite` v0.24.0 for its own native WebSocket implementation, while the project's own server-side `tokio-tungstenite` v0.30.0 dependency pulls a separate, newer `tungstenite` v0.30.0 - two different major versions of the same underlying crate coexist in the dependency tree, along with several of its own transitive dependencies duplicated in both versions (`block-buffer`, `digest`, `sha1`, and others - found via `cargo tree --duplicates`). No known functional impact - just extra compile time and binary size.

Check whether a newer `ewebsock` release has moved to a newer `tungstenite`, and upgrade if so and if it doesn't require API changes in `client_net.rs`. If `ewebsock` is genuinely stuck on the old version, there may be nothing to do here beyond documenting it as a known, currently-unfixable duplication rather than leaving it silently rediscoverable via `cargo tree` each time.

**Resolution (2026-09-14):** no newer `ewebsock` release exists to upgrade to. crates.io confirms `0.8.0` (published November 2024) is still both the `newest_version` and `max_version` - no release since. Checked upstream's unreleased `main` branch on GitHub too: it's since bumped its own dependency to `tungstenite = "0.29"`, closer to our `tokio-tungstenite`'s `0.30.0` but still not a match, and it isn't published to crates.io regardless - depending on an unpublished git revision for this would trade a cosmetic dependency-duplication issue for a real one (an unpinned, unreleased upstream dependency), which isn't a reasonable trade for a "no known functional impact" problem. Documented as a known, currently-unfixable duplication via a comment in `Cargo.toml` next to the `ewebsock` dependency, noting the unreleased `main`-branch progress so a future revisit knows where to check first.

## Acceptance criteria

- [x] Confirmed whether a newer `ewebsock` version resolves the `tungstenite` duplication - it doesn't; none exists (see resolution above)
- [x] If an upgrade is available and doesn't require API changes in `client_net.rs`, it's applied and `cargo tree --duplicates` shows the duplication resolved - n/a, no upgrade available
- [x] If no upgrade is available, this is documented as a known, currently-unfixable duplication (e.g. a comment near the `ewebsock` dependency in `Cargo.toml`) rather than left silently rediscoverable - done

## Blocked by

None - can start immediately.

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/46
