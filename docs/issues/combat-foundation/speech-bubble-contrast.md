## Parent

docs/prd/combat-foundation-m0-m1.md

## What to build

Fix the Motivational Speech speech-bubble's readability - reported directly as "colored in a very hard to read color."

Root cause (diagnosed, not just observed): the bubble is currently plain white `Text2d` with no background of any kind (`match_characters.rs`), unlike the Health HUD, which already solved the exact same problem - plain text in a single fixed color can end up nearly invisible against the Stage's own light-wall/dark-floor backdrop depending on where a Character is standing - with a dark background panel behind the text (see `health_hud.rs`'s own comment on why it has one). The speech bubble needs the same treatment: a solid or semi-transparent panel behind the text that guarantees contrast regardless of what's behind it, rather than a different single text color that could just as easily fail against some other part of the Stage.

## Acceptance criteria

- [ ] The speech bubble's text is clearly readable regardless of where on the Stage the casting Character is standing
- [ ] The fix follows the same "background panel behind text" approach already established by the Health HUD, for visual consistency across the client's text overlays
- [ ] Verified by casting Motivational Speech at multiple positions on the Stage (near a wall, near the floor band) and confirming the text stays legible in each

## Blocked by

None - can start immediately (`motivational-speech-special.md`, already done, is what this fixes).
