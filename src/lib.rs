//! Shared library crate: modules used by both the `client` (default) binary
//! and the `server` binary. See `docs/prd/combat-foundation-m0-m1.md` for
//! the module boundaries this follows.

pub mod combat;
pub mod net_protocol;

// A browser tab has no socket-listening API, so the server-side networking
// module can never compile for wasm32 - it isn't just unused there, it
// doesn't exist there at all.
#[cfg(not(target_arch = "wasm32"))]
pub mod server_net;
