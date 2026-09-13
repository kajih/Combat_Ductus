## What to build

Add structured logging across both the `client` and `server` binaries, using Bevy's `LogPlugin` (already part of `DefaultPlugins` for the client) and a matching setup for the headless server. Three levels matter, each for a different purpose:

- **error** — real failures: a failed asset load, a refused/dropped connection, a bind failure, an unexpected disconnect. Should already be the default for genuine error conditions, but needs to actually be checked/covered everywhere one of these can happen, not assumed.
- **info** — notable lifecycle/gameplay events: the server logging a client connecting/disconnecting and a Match ending (including the winner); the client logging its own connection lifecycle (connecting, connected, disconnected/failed).
- **trace** — a verbose, opt-in firehose for active debugging: every asset load attempt/success/failure, every input event sent/received, every state snapshot broadcast/received. The point of this level specifically is answering questions like "did the assets actually load in the browser" from the console alone, instead of guessing.

Needs to work on both native and the wasm/Trunk build - wasm needs its console-visible logging path (Bevy's `LogPlugin` already routes to the browser console on web), and the log level/filter should be adjustable without a rebuild where practical (`RUST_LOG` for native; wasm has no env vars, so this may need a URL query param or similar, or just a sensibly verbose default filter until there's a better mechanism).

## Acceptance criteria

- [ ] Client and server both emit `error!` logs for real failures (asset load failure, connection error/refusal, bind failure)
- [ ] Server emits `info!` logs when a client connects/disconnects and when a Match ends (including the winner)
- [ ] Client emits `info!` logs for its own connection lifecycle (connecting, connected, disconnected/failed)
- [ ] A trace-level log exists for asset load attempts/results, input events, and snapshot send/receive - verbose enough that "did this asset load" is answerable from the console alone
- [ ] Verified by running both the native build and the wasm/Trunk build and confirming all three levels appear appropriately in their respective console/terminal output

## Blocked by

None - can start immediately

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/12
