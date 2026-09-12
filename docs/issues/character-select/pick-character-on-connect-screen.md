## What to build

Add a character-selection step to the Connect screen (or immediately after connecting, before entering the Match) letting each player pick which Roster Character to play, replacing M0/M1's hardcoded Body Type for every player slot.

This is cheaper than it might sound: Body Type is purely cosmetic (ADR 0003) and every Roster Character already shares the exact same moveset (Punch/Kick/Motivational Speech, per an earlier design decision) - picking a Character just means picking which Body Type (and, eventually, whose face photo - a separate, optional, indefinitely-deferred track) to spawn with, not any new combat logic. `character_rig` already supports all three Body Types; this issue is really about the selection UI and getting the player's choice from client to server to spawn, not new rig/combat work.

Not relevant yet as of this writing - filed for later, not blocking any current work.

## Acceptance criteria

- [ ] The Connect screen (or a step immediately after connecting) presents the Roster's Characters for the player to choose from
- [ ] The chosen Body Type is what actually spawns for that player in the Match, replacing the currently-hardcoded `BodyType::Medium` for every player
- [ ] Selecting a Character requires no new per-Character combat logic - every Roster entry still shares the exact same moveset
- [ ] Verified by two different Body Type selections producing visibly different Characters in the same Match

## Blocked by

None - can start immediately (`client-connect-and-lifecycle.md` and `character-rig-compositing.md`, both already done, cover everything this needs).
