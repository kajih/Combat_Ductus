//! The headless `server` binary's guts: binds a WebSocket listener, steps
//! the authoritative `combat::MatchState` every tick regardless of whether
//! anyone is connected, applies a connected client's input, and broadcasts
//! a `net_protocol::StateSnapshot` to every connected client every tick.
//!
//! This module only exists on native targets (see `lib.rs`) - a browser tab
//! has no socket-listening API, so there is no wasm build to gate around
//! here, unlike most of this crate's other cfg-gating.
//!
//! Two halves run concurrently on separate OS threads, one per submodule:
//! - [`accept`] - a small tokio runtime drives the WebSocket accept loop and
//!   per-client read/write tasks (`accept::spawn_network_thread`,
//!   re-exported below)
//! - [`simulation`] - a headless Bevy `App` (`MinimalPlugins`, no rendering)
//!   steps the simulation on a fixed ~30Hz schedule
//!   (`simulation::run_bevy_app`, re-exported below)
//!
//! They talk to each other over a few plain channels, all defined here at
//! the module root since both halves need them: incoming `InputEvent`s flow
//! from the network thread into the Bevy world via a `crossbeam_channel`,
//! tagged with which player slot sent them (see `accept::SlotAssignment`);
//! outgoing `StateSnapshot` JSON flows out via a `tokio::sync::broadcast`
//! channel (which also gives spectator support - a 3rd+ connection - for
//! free, since any number of receivers can subscribe to the same broadcast,
//! and simply never gets a player slot assigned - see
//! `accept::assign_slot`); and a connection ending (closed, errored, or the
//! peer process killed outright) signals the simulation over a third
//! `crossbeam_channel` (see `Disconnection`), tagged with which slot (if
//! any) that connection held, so it can reset whatever movement that
//! connection last held and, if its opponent is still a real connected
//! client, declare a forfeit win for them - see
//! `docs/issues/combat-foundation/reset-input-on-disconnect.md`,
//! `docs/issues/combat-foundation/second-client-controls-player-two.md`,
//! and `docs/issues/combat-foundation/forfeit-win-on-disconnect.md`.
//!
//! This is one module split across a folder (`accept.rs`/`simulation.rs`/
//! `tests.rs` alongside this file, per
//! `docs/issues/combat-foundation/split-server-net-test-module.md`), not
//! several independent ones - everything outside this module still only
//! ever sees `server_net::spawn_network_thread`/`server_net::run_bevy_app`,
//! re-exported below, never the submodules themselves.

mod accept;
mod simulation;
#[cfg(test)]
mod tests;

pub use accept::spawn_network_thread;
pub use simulation::run_bevy_app;

use crate::combat::Player;
use crate::net_protocol::InputEvent;
use crossbeam_channel::Receiver as CbReceiver;
use std::net::SocketAddr;
use tokio::sync::broadcast;

/// An input event tagged with which player slot sent it, if any. `None`
/// means the sending connection is a spectator (a 3rd+ connection - see
/// `accept::assign_slot`) with no Character of its own to control;
/// movement/attack events from one of those are simply ignored, and so is
/// `RequestRestart`, restricted to a real player slot per
/// `docs/issues/combat-foundation/restrict-restart-to-players.md` since a
/// spectator has no stake in the Match it would be restarting.
type TaggedInputEvent = (Option<Player>, InputEvent);

/// What happened when one connection ended, for the simulation to react
/// to. `opponent_connected` is only meaningful when `slot` is `Some` - a
/// spectator (`slot: None`) disconnecting never triggers a forfeit,
/// regardless of who else is connected, since it never held a Character to
/// begin with. See
/// `docs/issues/combat-foundation/forfeit-win-on-disconnect.md`.
#[derive(Debug, Clone, Copy)]
pub struct Disconnection {
    /// Which player slot (if any) the ended connection held - `None` means
    /// it was a spectator.
    pub slot: Option<Player>,
    /// Whether the *other* player slot was still held by a real, settled
    /// connected client (see `accept::SlotAssignment::is_settled`) at the
    /// instant this connection ended - the fact a forfeit hinges on, per
    /// `forfeit-win-on-disconnect.md`'s decision that a disconnect only
    /// forfeits the Match when both slots were real, connected players.
    pub opponent_connected: bool,
}

/// The pieces a caller needs to run the server: the address it actually
/// bound to (useful when binding to port 0), and the channels connecting
/// network I/O to the simulation.
pub struct ServerParts {
    pub local_addr: SocketAddr,
    pub incoming_rx: CbReceiver<TaggedInputEvent>,
    pub outgoing_tx: broadcast::Sender<String>,
    /// Fires once per connection, the moment that connection is fully gone
    /// (clean close, error, or an abrupt drop) - see `Disconnection`.
    pub disconnected_rx: CbReceiver<Disconnection>,
}
