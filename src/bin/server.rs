//! The headless, native-only server binary. Runs the one authoritative
//! `combat::MatchState` and streams it to connected clients over
//! WebSockets. See `docs/issues/combat-foundation/server-skeleton.md`.
//!
//! Every player - including whoever hosts - connects to this as a
//! `client` (browser/wasm) build; this binary never renders anything and
//! is never itself the wasm build (a browser tab cannot bind a listening
//! socket, which is the whole reason this is a separate process).
//!
//! `main` itself is `cfg`-gated, not just `server_net` (which it calls):
//! `cargo build --target wasm32-unknown-unknown` with no `--bin` filter -
//! which is exactly what `trunk build`/`trunk serve` run under the hood -
//! builds every binary in the package for that target, this one included.
//! Referencing `server_net` unconditionally would fail to compile there
//! even though nothing would ever actually run this binary on wasm.

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    let bind_addr =
        std::env::var("COMBAT_DUCTUS_SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:9000".to_string());

    let parts = combat_ductus::server_net::spawn_network_thread(&bind_addr)
        .expect("failed to start server networking");

    println!("combat_ductus server listening on {}", parts.local_addr);

    // Blocks forever, stepping the simulation on a fixed schedule.
    combat_ductus::server_net::run_bevy_app(parts.incoming_rx, parts.outgoing_tx);
}

#[cfg(target_arch = "wasm32")]
fn main() {
    panic!(
        "the server binary does not run on wasm32 - a browser tab cannot bind a listening socket"
    );
}
