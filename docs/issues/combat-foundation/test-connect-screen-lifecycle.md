## Parent

docs/issues/combat-foundation/client-connect-and-lifecycle.md (done - this adds missing test coverage for it, not new behavior)

## What to build

`connect_screen.rs`'s connection-lifecycle state machine (`Connecting` <-> `InMatch` <-> `MatchEnded`, reconnect/disconnect handling, and the `MyRole` reset-on-disconnect logic added in `connection-identity-indicator.md`) is genuinely stateful and has grown more complex over successive issues, but has zero automated tests today - it's only ever been verified by manual testing (clicking Connect, killing servers, watching screenshots). Add test coverage for it.

Remaining open branch: exactly how to test Bevy systems that depend on `NonSendMut<ConnectionSlot>` and a live `Connection` - probably either a fake/mock `Connection` implementation injectable in tests, or driving a real `App` with `MinimalPlugins` against a real local server the way `server_net.rs`'s own tests already do (`spawn_network_thread` + `run_bevy_app`) - left to whoever implements this to find the cleanest seam. Worth checking in on `split-server-net-test-module.md` first, since whatever test-infrastructure seam that produces might be reusable here.

## Acceptance criteria

- [ ] A disconnect (server closes, or the server process dies) correctly transitions `AppState` back to `Connecting` and resets `MyRole` to `Unknown` - verified by an automated test, not just manual testing
- [ ] Reconnecting after a disconnect gets a fresh `MyRole` reflecting whatever slot the new connection actually receives
- [ ] `ConnectionSlot::send_input`'s existing "silently do nothing before a connection exists" behavior is covered

## Blocked by

None - though this benefits from whatever test-infrastructure seam `split-server-net-test-module.md` produces, if that lands first (not a hard dependency).

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/45
