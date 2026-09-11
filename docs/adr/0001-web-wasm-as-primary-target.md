# Web (WASM) is the primary target platform

This repo already has a fast native dev build set up (dynamic linking, split opt-levels, `rust-lld`) as well as a Trunk/WASM web build. For an internal demo game, we're prioritizing shareability over native performance or gamepad support: playtesters get a browser link instead of a binary to download and run. The web (WASM via Trunk) build is therefore the primary target platform; the native build remains in place mainly to keep local development iteration fast, not as the shipped target.
