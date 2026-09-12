# Combat Foundation: M0 (Rendered Character) & M1 (Idle Opponent)

## Problem Statement

`src/main.rs` currently opens an empty Bevy window — there is no ECS setup, no rendering of a Character, no combat logic, and no networking. The domain model (`CONTEXT.md`) and eight ADRs describe *what* the game is (a 1v1 versus fighter with fixed Facing, no block/dodge, cosmetic-only Body Types, layered body-part sprite compositing, server-authoritative networking, and a Motivational Speech Special), but none of it has been built, and several load-bearing implementation questions were still open: how the client and server topology actually works given a browser cannot host a listening socket, what the input scheme is, how hits are detected, and what is and isn't in scope for a first playable slice.

Without resolving these, no concrete implementation task can be written with confidence that it won't be invalidated by a later architectural correction (as happened here: the original assumption that "the wasm build is both client and server" turned out to be technically impossible, since a browser tab cannot accept incoming connections).

## Solution

Establish the foundational architecture and ship two small, sequential milestones that de-risk the two riskiest unknowns independently before any further feature work is planned:

- **M0 — Rendered Character:** prove the layered body-part sprite compositing pipeline (ADR 0007) actually works end-to-end for one Character on the Stage, with no input, combat, or networking involved.
- **M1 — Idle Opponent:** prove the server-authoritative combat loop (ADR 0005) end-to-end for one real client — movement, Punch/Kick, Health depletion, and a terminal Match-end screen — against a stationary, unclaimed Player 2 (the Idle Opponent).

Both milestones share one crate with a clear module boundary between pure combat rules, shared network message types, the client binary, and the server binary, so that combat logic is written and tested once and never duplicated between the authoritative simulation and any future tooling.

## User Stories

1. As a developer, I want a Character composited at runtme from separate torso/arm/leg sprites per (Body Type, Facing), so that adding a new attack or Body Type never requires a new whole-body frame.
2. As a developer, I want the face-photo slot to accept a placeholder image, so that engine/rendering work is never blocked on sourcing real employee photos or consent.
3. As a player, I want to see my Character idle on the Stage with correct proportions for its Body Type, so that I can confirm the art pipeline renders recognizable, correctly-scaled Characters.
4. As a developer, I want combat rules (damage, range checks, Special gating, Match-end) implemented as pure functions with no Bevy `App` dependency, so that they can be unit-tested in isolation from rendering and networking.
5. As a developer, I want a shared `net_protocol` module of plain serde-serializable message types, so that the client and server never drift on wire format and the same types compile on both wasm and native targets.
6. As a host player, I want to run a native, headless server process separate from my browser client, so that the server can bind a listening WebSocket socket, which no browser tab is able to do.
7. As any player, I want to open the client in a browser (served via Trunk) and see a connect screen where I can type in the host's LAN IP, so that I can join a Match without knowing the server's address in advance or rebuilding anything.
8. As the second player to connect, I want to take over control of the previously-idle Player 2 Character the moment I connect, so that the Match becomes a real 1v1 contest per the Idle Opponent definition.
9. As a player, I want to move left/right with A/D, jump with Space, Punch with J, and Kick with K, so that I have simple, discoverable controls with no on-screen prompt needed.
10. As a player, I want my Punch/Kick to hit the opponent only when they're within a fixed range in my fixed Facing direction, so that positioning is the only way to avoid or land an attack, per ADR 0004.
11. As a player, I want to be unable to Punch or Kick while airborne, so that Jump remains purely a repositioning tool, per the existing Jump definition.
12. As a player, I want the Match to end immediately and show a static "X wins" screen the instant either Character's Health reaches 0, so that the terminal Match-end behavior in `CONTEXT.md` is honored (no rounds, no rematch flow in v1).
13. As a player, I want no match timer of any kind, so that a Match only ever ends by Health reaching 0, regardless of duration.
14. As a developer, I want the server to be the sole source of simulation truth every tick, streaming state snapshots to the client, so that the client never needs to reconcile a diverging local simulation, per ADR 0005.
15. As a developer, I want the server binary to be `cfg`-gated out of wasm compilation entirely, so that the wasm client build never attempts to pull in socket-listening code that couldn't compile there anyway.
16. As a developer, I want the server to run as a headless Bevy app (`MinimalPlugins`, no rendering/window/audio), so that it reuses the same `combat` module and Bevy ECS patterns as the client without any rendering overhead.
17. As a QA/playtester, I want to run the game entirely over the office LAN with manually-entered IPs, so that no internet-facing hosting, discovery, or matchmaking work is needed for v1.
18. As a developer, I want each Roster Character to use the exact same moveset (Punch, Kick, Motivational Speech with identical damage/range/gating), so that Body Type remains the only per-Character axis of engine work, matching its cosmetic-only status (ADR 0003).
19. As a developer, I want Motivational Speech's per-Character variation limited to its display text, so that adding a new Roster Character never requires new combat balancing, animation, or voice recording.
20. As a developer, I want the Motivational Speech speech-bubble VFX to display a Character-specific text line, so that the Special reflects personality without requiring audio production.
21. As a developer, I want no SFX or music in scope for M0/M1, so that audio work never blocks or delays the core combat/networking loop.
22. As a developer, I want no character-select screen in M0/M1, so that each player slot can be hardcoded to a fixed Character while the roster/photo pipeline remains a separate, optional track.

## Implementation Decisions

**Module boundaries** (see PRD conversation for full rationale):

- **`combat`** — the deep module. Pure Rust: Health, damage application (Punch = 1, Kick = 2), the 1D range check per attack, Special gating (minimum cast range, cooldown, recent-damage lockout per ADR 0008), and Match-end detection (Health reaches 0). No Bevy `App`, no rendering, no networking — takes simple state (positions, Facing, timers) and simple inputs, returns simple results. This is the only module carrying unit tests.
- **`net_protocol`** — shared message types only: client→server input events (e.g. "Punch pressed this tick," movement direction) and server→state snapshots (positions, Health, Facing, Match status). Plain structs/enums + `serde`, JSON-serialized over WebSocket. Compiles identically on `wasm32-unknown-unknown` and native since it does no I/O itself.
- **`server`** (binary/module) — headless Bevy app using `MinimalPlugins`. Owns the one authoritative `combat` simulation, runs a WebSocket listener (e.g. `tokio-tungstenite`), tracks each player slot's state (Idle Opponent vs. connected client), broadcasts state snapshots every tick. `#[cfg(not(target_arch = "wasm32"))]`-gated — a browser tab cannot bind a listening socket, so this can never be part of the wasm build.
- **`character_rig`** (client-only) — composites a Character from torso/arm/leg sprite entities attached at socket offsets keyed by (Body Type, Facing), per ADR 0007, including the face-photo slot and its horizontal squash transform per ADR 0006. Fed a placeholder photo for all Characters until/unless the real-photo track (out of scope here) delivers actual images.
- **`client`** (binary; this is the existing `main.rs`) — `DefaultPlugins`, built both as the fast native dev build and as the Trunk/wasm web build (per ADR 0001). Owns app-level states: Connect screen → In-Match → Match-Ended. Captures input (A/D move, Space jump, J punch, K kick, H special), sends input events to the server via `net_protocol`, renders both Characters from received state snapshots via `character_rig`. Never runs its own simulation — pure renderer, per ADR 0005.

**Networking topology** (the key correction made during design discussion):

- Transport is plain WebSockets with JSON payloads — no networking crate, no client-side prediction/rollback/reconciliation machinery. This matches ADR 0005's premise that the client never has an independent "opinion" to reconcile.
- The server is necessarily a **separate native process** from any browser tab, because WASM running in a browser has no socket-listening API — it can only make outbound connections. "The wasm build acting as both client and server in the same browser tab" is not achievable with this transport (or any browser-native transport) and was ruled out during design.
- Every player, including whoever hosts, connects as a browser/wasm client. The host additionally runs the native `server` binary as a background process before anyone connects.
- v1 connection flow is fully manual: a connect screen with a plain IP text field, no discovery/matchmaking/signaling. LAN-only, permanently — this keeps ADR 0005's "imperceptible latency" justification valid; internet-facing hosting is explicitly out of scope and would require revisiting ADR 0005 if ever pursued.
- Spectators (a 3rd+ connection) were discussed as a plausible easy extension of the server's per-slot connection tracking, but no concrete design was made — tracked as a future consideration, not part of M0/M1.

**Milestones:**

- **M0 — Rendered Character:** one Character (one Body Type, placeholder face photo) rigged via `character_rig` and idle on the Stage, correct camera, no input/combat/networking of any kind. Validates the ADR 0007 compositing pipeline in isolation.
- **M1 — Idle Opponent:** the `client`/`server`/`net_protocol`/`combat` loop working end-to-end for one real connected client: movement (A/D/Space), Punch/Kick against a stationary Player 2, Health depletion via `combat`, and a static Match-end screen. Player 2 remains an Idle Opponent throughout M1 — a second real client connecting is out of scope until a later milestone.

**Combat rules fixed for M0/M1:**

- Hit detection is a 1D distance/range check per attack (not hitbox/hurtbox rectangles) — is the opponent within attack range, in the attacker's fixed Facing direction, and not airborne.
- No match timer of any kind; only the existing per-Character cooldown and recent-damage-lockout timers for Motivational Speech (ADR 0008). Exact numeric thresholds (ranges, cooldown/lockout seconds) are tuning values, deferred.
- Jump blocks Punch/Kick while airborne (existing Jump definition); exact milestone placement of Jump implementation within M0/M1's task breakdown is left to task design.
- All Roster Characters share one identical moveset; Motivational Speech's only per-Character variation is its display text (no per-Character balancing, animation, or audio).
- No character-select screen; each player slot is hardcoded to a fixed Character for M0/M1.

**Content/audio decisions:**

- Real employee face photos and consent-gathering are an explicit, optional, non-blocking side track — engine work never depends on them landing.
- Audio is text-only for M0/M1: the Motivational Speech speech-bubble VFX displays a per-Character text line. No SFX (impacts, jump, etc.) and no music in scope.

## Testing Decisions

- Only the `combat` module carries unit tests. Because it's pure Rust with no Bevy `App`, `#[test]` functions can exercise it directly with plain inputs and assertions — no test harness or Bevy test app needed.
- Test coverage should include: damage application (Punch = 1, Kick = 2, Health floor behavior at/below 0), range-check boundaries (exactly at range, just inside, just outside), Special gating conditions (at the minimum-range boundary, during cooldown, during the recent-damage lockout window, and the combination of all three), and Match-end triggering (Health reaches exactly 0, does not trigger above 0).
- `character_rig`, `net_protocol`, `server`, and `client` are not unit-tested for M0/M1 — they're validated by running the game (M0: does the Character render correctly on screen; M1: does a real client's Punch/Kick actually deplete the opponent's Health and end the Match). This matches the project's current stage — there is no existing test-writing prior art in the repo to follow (no tests exist yet), so `combat`'s tests establish the first convention.

## Out of Scope

- A second real client connecting (both players controlled) — M1 keeps Player 2 as a permanent Idle Opponent; real two-client play is a later milestone.
- Character select screen, and any roster content beyond one hardcoded Character per player slot. Still out of scope for this PRD's M0/M1 milestones, but now tracked as a future feature — see `docs/issues/character-select/pick-character-on-connect-screen.md`.
- Real employee face photos, consent, and any photo-sourcing pipeline.
- Voiced/recorded audio, sound effects, and music.
- Hitbox/hurtbox rectangle collision (only the simpler 1D range check is in scope).
- A match timer, draw conditions, or any time-based win condition.
- ~~Rematch/restart flow after a Match ends.~~ **Superseded**: real playtesting of `punch-kick-health-depletion` showed that ending a Match with zero way to continue (refresh-to-restart) is a real gap, not an acceptable simplification. See `docs/issues/combat-foundation/restart-match.md`.
- Spectator mode (3rd+ connections) — noted as a plausible future extension of the server's connection tracking, not designed here. Now tracked as a future feature — see `docs/issues/spectator-mode/spectator-connections.md`.
- Internet-facing hosting, matchmaking, or server discovery — LAN-only with manual IP entry is permanent for this project's scope, not a stepping stone.
- Client-side prediction, rollback, or any reconciliation logic — the client is a pure renderer per ADR 0005.
- Exact tuning values (attack ranges, cooldown/lockout durations) — left as placeholder constants to be tuned during/after implementation.

## Further Notes

- The genre definition in `CONTEXT.md` ("Versus Fighter... won by depleting the opponent's health before time runs out") is inherited boilerplate that implies a match timer that does not exist anywhere else in the domain model or ADRs. Worth fixing that wording in a follow-up edit to `CONTEXT.md` so it stops implying a mechanic the game doesn't have.
- The corrected networking topology (separate native headless server, all players as browser clients, LAN-only) is a strong candidate for its own ADR, since it revises the implicit assumption in how ADR 0005 has been read up to now (it doesn't contradict ADR 0005's content, but the "wasm is both client and server" idea that was ruled out during this design pass was never written down anywhere and could easily resurface as a false assumption in a future session).
- All of the existing 8 ADRs remain valid and unmodified by this PRD; this document only adds the previously-missing implementation-level decisions needed to start writing M0/M1 tasks.
