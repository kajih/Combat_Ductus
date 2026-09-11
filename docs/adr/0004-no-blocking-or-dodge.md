# No blocking or dedicated dodge mechanic

Nearly all fighting games include some form of blocking or guarding. combat_ductus deliberately has neither a block input nor a dedicated dodge/invincibility mechanic — the only way to avoid an attack is to move out of its range. This keeps the input and state-machine surface area minimal for a first build: defense-by-positioning needs no extra animations, block-stun handling, or chip-damage rules that a real block system would require.
