## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

The client-side connect screen and WebSocket connection lifecycle. On launch, the client shows a Connect screen with a plain text field for the server's LAN IP. On submit, it opens a WebSocket connection to that address, and on success transitions the client's app state from Connecting to In-Match. Incoming `net_protocol` state snapshots are received and stored for later rendering work; a failed/refused connection keeps the user on the Connect screen with visible feedback.

## Acceptance criteria

- [ ] The Connect screen renders with an editable IP text field and a connect action
- [ ] Submitting a valid server address opens a WebSocket connection and, on success, transitions the client out of the Connect screen
- [ ] Incoming state snapshots are deserialized via `net_protocol` and stored somewhere accessible to later rendering work
- [ ] A failed/refused connection keeps the client on the Connect screen with some visible indication of failure
- [ ] Verified by running the client against the server skeleton and confirming the state transition and snapshot receipt (logged output is sufficient — no rendering required yet)

## Blocked by

- docs/issues/combat-foundation/net-protocol-messages.md
- docs/issues/combat-foundation/server-skeleton.md

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/8
