# Characters have fixed facing; they never turn

Classic versus fighters (Street Fighter, Mortal Kombat) turn each character to always face their opponent. We deliberately dropped this: each Character's Facing is fixed for the whole Match — Player 1 always faces right, Player 2 always faces left — regardless of either Character's position on the Stage. This keeps input handling, animation, and attack-direction logic trivial for a first, intentionally minimal build. Adding opponent-tracking facing later is a contained addition, not a redesign, if it turns out to be worth the extra complexity.
