## Parent

`docs/issues/combat-foundation/client-connect-and-lifecycle.md` - follow-up to the Connect screen's default address, changed in PR #54.

## Priority

Low. The change typechecks on both targets and the reasoning is straightforward; this is confirmation of a small runtime behaviour, not suspected-broken work. Worth doing on the next LAN playtest rather than scheduling deliberately.

## What to build

Nothing to build - this is a verification task.

`connect_screen::default_server_address` now derives the Connect screen's pre-filled address from the host that served the page (`web_sys::window().location().hostname()`), instead of a hardcoded `127.0.0.1:9000` that meant "this browser's own machine" on every computer except the host's. It compiles for both `wasm32-unknown-unknown` and native, and `cargo clippy`/`cargo fmt` are clean, but the pre-filled value has never actually been observed in a running browser - the change was made and verified at the end of a session, with no LAN playtest afterwards.

Two things in particular are worth eyeballing rather than assuming:

- Whether `hostname()` returns what's expected when the page is loaded by IP rather than by name. It should be the bare host with no port (`192.168.1.112`), which is why the port is appended separately - but that's reasoning, not observation.
- Whether the native build still shows the loopback fallback. The `#[cfg(target_arch = "wasm32")]` block is compiled out there, so it should, but the two paths have only been exercised by the compiler.

If `hostname()` turns out to be empty or unexpected in some browser, the fallback to `127.0.0.1:9000` already covers it without breaking the screen - so the realistic failure mode is "no better than before", not a regression.

## Acceptance criteria

- [ ] Loading the client from another machine at `http://<host-LAN-IP>:8080` shows `<host-LAN-IP>:9000` already in the address field
- [ ] Pressing Connect with that untouched default reaches the server and starts a Match, with no manual typing
- [ ] The native client (`cargo run --features dev`) still defaults to `127.0.0.1:9000`
- [ ] If any of the above is wrong, `default_server_address` is corrected and this issue records what the browser actually returned

## Blocked by

- PR #54 (`lan-play-from-other-machines`) being merged - that's what introduces the behaviour to verify

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/55
