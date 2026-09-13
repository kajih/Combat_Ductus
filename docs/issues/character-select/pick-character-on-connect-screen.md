## What to build

Add a character-selection step to the Connect screen (or immediately after connecting, before entering the Match) letting each player pick which Roster Character to play, replacing M0/M1's hardcoded Body Type for every player slot.

This is cheaper than it might sound: Body Type is purely cosmetic (ADR 0003) and every Roster Character already shares the exact same moveset (Punch/Kick/Motivational Speech, per an earlier design decision) - picking a Character just means picking which Body Type (and, eventually, whose face photo - a separate, optional, indefinitely-deferred track) to spawn with, not any new combat logic. `character_rig` already supports all three Body Types; this issue is really about the selection UI and getting the player's choice from client to server to spawn, not new rig/combat work.

**Finding from design review**: this is more than a client-side picker screen. `net_protocol::CharacterSnapshot` has no `body_type` field at all, and `combat::CharacterState` doesn't track it either - both clients currently just hardcode `BodyType::Medium`. This needs a new place to store "which BodyType did this slot choose" server-side (likely `server_net.rs`-level state, not `combat::MatchState` itself, since Body Type is cosmetic-only per ADR 0003 and the pure combat rules have no reason to know it), plus a new wire field so the *other* client can render the choice correctly.

**Decided: selection happens after connecting, not before.** Connect first (server assigns P1/P2/spectator as today via `second-client-controls-player-two.md`), then show the picker once the connection knows its own role, sending the choice as a new input event. Picking before connecting would mean running the picker with no server connection yet, and without knowing in advance whether this connection will even get a real player slot.

## Acceptance criteria

- [ ] A step after connecting (once the connection knows it holds a real player slot) presents the Roster's Characters for that player to choose from
- [ ] A spectator connection never sees the picker at all
- [ ] The chosen Body Type is what actually spawns for that player in the Match, replacing the currently-hardcoded `BodyType::Medium`, and is visible to every other connected client via a new wire field
- [ ] Selecting a Character requires no new per-Character combat logic - every Roster entry still shares the exact same moveset
- [ ] Verified by two different Body Type selections producing visibly different Characters in the same Match, as seen by both clients

## Blocked by

- docs/issues/combat-foundation/connection-identity-indicator.md (not yet built - a connection needs to know it holds a real player slot, and not a spectator, before the picker can decide whether to show itself)

`client-connect-and-lifecycle.md` and `character-rig-compositing.md`, both already done, cover everything else this needs.
