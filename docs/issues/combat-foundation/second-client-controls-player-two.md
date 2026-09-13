## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

Let a second real client connect and take control of Player 2, replacing the Idle Opponent state the moment that second connection arrives - exactly matching the existing Idle Opponent definition ("resolves into a normal controlled Character the moment a second Client joins"). Everything built so far is genuinely a 1v1 versus fighter played against a Character that never fights back; this is the single biggest remaining gap toward it actually being a duel between two people.

This needs the server to assign connections to player slots (first connection = Player 1, second = Player 2) instead of unconditionally treating all incoming input as Player 1's, and to apply Player 2's input to actually move and attack as Player 2. This is also the natural foundation `spectator-mode/spectator-connections.md` needs (a 3rd+ connection becomes a spectator once both player slots are taken) - the two issues likely share the same connection/role-assignment groundwork, so worth designing together even if built separately.

## Acceptance criteria

- [x] A first connection is assigned Player 1, as today
- [x] A second connection is assigned Player 2, and its A/D/J/K input actually moves and attacks with Player 2, instead of being ignored or misapplied to Player 1
- [x] The moment a second client connects, Player 2 stops behaving as the stationary Idle Opponent and becomes a normal controlled Character
- [x] If the second client disconnects, Player 2 reverts to being a stationary Idle Opponent again rather than continuing to move/attack on its own
- [ ] Verified by connecting two separate clients simultaneously and confirming each independently controls their own Character in a real two-sided Match

## Blocked by

- docs/issues/combat-foundation/reset-input-on-disconnect.md

## GitHub Issue

https://github.com/kajih/Combat_Ductus/issues/20
