## Parent

docs/issues/combat-foundation/client-connect-and-lifecycle.md (done - this adds missing test coverage for it, not new behavior)

## What to build

`connect_screen.rs`'s connection-lifecycle state machine (`Connecting` <-> `InMatch` <-> `MatchEnded`, reconnect/disconnect handling, and the `MyRole` reset-on-disconnect logic added in `connection-identity-indicator.md`) is genuinely stateful and has grown more complex over successive issues, but has zero automated tests today - it's only ever been verified by manual testing (clicking Connect, killing servers, watching screenshots). Add test coverage for it.

Resolved (the seam this was left open on): the real-server route, not a mock. `Connection` holds an `ewebsock::WsSender`, which has no public constructor - faking one would have meant reshaping production code purely to be mocked, and would have made the reconnect criterion meaningless besides, since the slot a reconnection gets would then be one the test decided on rather than one the real slot assignment handed out. So: the real `ConnectScreenPlugin` on a headless `App` (`MinimalPlugins` + `StatesPlugin`), stepped one `update()` at a time, connected to a real `spawn_network_thread`/`run_bevy_app` server. The non-send `ConnectionSlot` needs no special handling - the test thread creates the `World` and steps it, so it *is* that world's main thread.

The one gap that needed filling: a real server can't be made to die on command (`spawn_network_thread` returns no shutdown handle), so the tests reach it through `CuttableLink`, a dumb TCP relay they sever mid-Match. Cutting it drops the connection mid-frame and unannounced, in both directions at once - which is what a killed server process looks like to the client, and what frees the slot again on the server side.

Production code is unchanged apart from the `mod tests;` declaration: this was missing coverage, not missing behavior. `split-server-net-test-module.md` landed first, but produced no reusable seam - its helpers are `tokio-tungstenite` clients driving the server's wire protocol, a different job from driving the client's own Bevy app.

## Acceptance criteria

- [x] A disconnect (server closes, or the server process dies) correctly transitions `AppState` back to `Connecting` and resets `MyRole` to `Unknown` - verified by an automated test, not just manual testing
- [x] Reconnecting after a disconnect gets a fresh `MyRole` reflecting whatever slot the new connection actually receives
- [x] `ConnectionSlot::send_input`'s existing "silently do nothing before a connection exists" behavior is covered

## Blocked by

None - though this benefits from whatever test-infrastructure seam `split-server-net-test-module.md` produces, if that lands first (not a hard dependency).

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/45
