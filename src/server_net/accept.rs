//! The WebSocket accept loop: binds the listener, assigns each connection a
//! player slot (or spectator), and forwards its input/disconnect to the
//! simulation (`super::simulation`) over the channels `super` defines.
//! See the parent module's doc comment for the full picture.

use super::{Disconnection, ServerParts, TaggedInputEvent};
use crate::combat::Player;
use crate::net_protocol::{InputEvent, ServerMessage};
use crossbeam_channel::Sender as CbSender;
use futures_util::{SinkExt, StreamExt};
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

/// Which player slots are currently held by a connected client. Shared
/// between the accept loop and every in-flight connection task: the first
/// connection to arrive while a slot is free claims it (P1 before P2), and
/// releases it again the moment that connection ends, so the next
/// connection to arrive can take over the same slot - see
/// `docs/issues/combat-foundation/second-client-controls-player-two.md`.
/// A 3rd+ connection, arriving while both slots are taken, gets no slot at
/// all and becomes a spectator (still receives every broadcast snapshot,
/// same as any other connection, but its input is never applied to
/// either Character).
#[derive(Default)]
struct SlotAssignment {
    p1_taken: bool,
    p2_taken: bool,
    /// When each slot most recently transitioned from vacant to taken -
    /// `None` while vacant. Used only to debounce the forfeit check (see
    /// `is_settled`) - slot assignment itself (`assign_slot`) never
    /// consults this.
    p1_taken_since: Option<Instant>,
    p2_taken_since: Option<Instant>,
}

/// How long a slot must have been *continuously* taken before its occupant
/// counts as a real opponent for forfeit purposes. Guards against a narrow
/// but real race: detecting a connection's disconnect is never instant (it
/// takes a failed send on the *next* broadcast tick to notice at all), and
/// in that brief window an unrelated connection can claim the *other*,
/// already-vacant slot and vanish again before the original disconnect is
/// even processed - which, checked naively, reads identically to "the
/// opponent was real and connected." Real players never reconnect within
/// a fraction of a tick of each other; this only ever debounces that
/// impossible-for-a-human timing, not genuine gameplay. See
/// `docs/issues/combat-foundation/forfeit-win-on-disconnect.md`.
const MIN_OPPONENT_SETTLE: Duration = Duration::from_millis(75);

impl SlotAssignment {
    fn is_taken(&self, player: Player) -> bool {
        match player {
            Player::P1 => self.p1_taken,
            Player::P2 => self.p2_taken,
        }
    }

    fn set_taken(&mut self, player: Player, taken: bool) {
        let since = taken.then(Instant::now);
        match player {
            Player::P1 => {
                self.p1_taken = taken;
                self.p1_taken_since = since;
            }
            Player::P2 => {
                self.p2_taken = taken;
                self.p2_taken_since = since;
            }
        }
    }

    /// Whether `player`'s slot is not just taken, but has been taken for
    /// at least `MIN_OPPONENT_SETTLE` continuously - see its doc comment.
    fn is_settled(&self, player: Player) -> bool {
        let taken_since = match player {
            Player::P1 => self.p1_taken_since,
            Player::P2 => self.p2_taken_since,
        };
        taken_since.is_some_and(|since| since.elapsed() >= MIN_OPPONENT_SETTLE)
    }
}

/// The other of the two player slots - used at disconnect time to check
/// whether *that* slot is still held by a real connected client (see
/// `Disconnection::opponent_connected`).
fn other_player(player: Player) -> Player {
    match player {
        Player::P1 => Player::P2,
        Player::P2 => Player::P1,
    }
}

/// Claim the first free slot (P1 before P2), or `None` if both are already
/// taken (this connection is a spectator).
fn assign_slot(slots: &Mutex<SlotAssignment>) -> Option<Player> {
    let mut slots = slots.lock().expect("slot assignment mutex was poisoned");
    for player in [Player::P1, Player::P2] {
        if !slots.is_taken(player) {
            slots.set_taken(player, true);
            return Some(player);
        }
    }
    None
}

/// Bind a WebSocket listener at `bind_addr` and start accepting connections
/// on a background thread with its own tokio runtime. Returns as soon as
/// the listener is actually bound (so `bind_addr` can end in `:0` to let
/// the OS pick a free port, e.g. in tests).
pub fn spawn_network_thread(bind_addr: &str) -> io::Result<ServerParts> {
    let (incoming_tx, incoming_rx) = crossbeam_channel::unbounded::<TaggedInputEvent>();
    let (disconnected_tx, disconnected_rx) = crossbeam_channel::unbounded::<Disconnection>();
    let (outgoing_tx, _) = broadcast::channel::<String>(32);
    let outgoing_tx_for_net = outgoing_tx.clone();
    let bind_addr = bind_addr.to_string();
    let (addr_tx, addr_rx) = std::sync::mpsc::channel::<io::Result<SocketAddr>>();
    let slots = Arc::new(Mutex::new(SlotAssignment::default()));

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");
        runtime.block_on(async move {
            let listener = match TcpListener::bind(&bind_addr).await {
                Ok(listener) => listener,
                Err(err) => {
                    let _ = addr_tx.send(Err(err));
                    return;
                }
            };
            let local_addr = listener
                .local_addr()
                .expect("a bound listener has a local address");
            let _ = addr_tx.send(Ok(local_addr));

            accept_connections(
                listener,
                incoming_tx,
                outgoing_tx_for_net,
                disconnected_tx,
                slots,
            )
            .await;
        });
    });

    let local_addr = addr_rx
        .recv()
        .map_err(|_| io::Error::other("server network thread exited before binding"))??;

    Ok(ServerParts {
        local_addr,
        incoming_rx,
        outgoing_tx,
        disconnected_rx,
    })
}

/// Accept WebSocket connections forever, assigning each one a player slot
/// (first come, first served - see `assign_slot`), forwarding its incoming
/// input events (tagged with that slot) into `incoming_tx`, and forwarding
/// every broadcast snapshot out to that connection regardless of whether
/// it got a slot at all.
async fn accept_connections(
    listener: TcpListener,
    incoming_tx: CbSender<TaggedInputEvent>,
    outgoing: broadcast::Sender<String>,
    disconnected_tx: CbSender<Disconnection>,
    slots: Arc<Mutex<SlotAssignment>>,
) {
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(pair) => pair,
            Err(_) => continue,
        };

        let incoming_tx = incoming_tx.clone();
        let mut outgoing_rx = outgoing.subscribe();
        let disconnected_tx = disconnected_tx.clone();
        let slots = Arc::clone(&slots);
        let slot = assign_slot(&slots);

        tokio::spawn(async move {
            let ws_stream = match tokio_tungstenite::accept_async(stream).await {
                Ok(ws) => ws,
                Err(_) => {
                    // The connection never actually opened, so it never
                    // held its slot in any meaningful sense - free it back
                    // up immediately rather than leaking it forever.
                    if let Some(player) = slot {
                        slots
                            .lock()
                            .expect("slot assignment mutex was poisoned")
                            .set_taken(player, false);
                    }
                    return;
                }
            };
            let (mut write, mut read) = ws_stream.split();

            // A one-time message telling this connection its own role -
            // which player slot it holds, or that it's spectating - before
            // it ever starts forwarding the shared broadcast below. See
            // docs/issues/combat-foundation/connection-identity-indicator.md.
            let your_slot_json = serde_json::to_string(&ServerMessage::YourSlot(slot))
                .expect("ServerMessage always serializes");
            if write
                .send(Message::Text(your_slot_json.into()))
                .await
                .is_err()
            {
                // Gone before we could even tell it its own slot - free the
                // slot the same as an outright accept failure above.
                if let Some(player) = slot {
                    slots
                        .lock()
                        .expect("slot assignment mutex was poisoned")
                        .set_taken(player, false);
                }
                return;
            }

            let reader = tokio::spawn(async move {
                while let Some(Ok(message)) = read.next().await {
                    if let Message::Text(text) = message
                        && let Ok(event) = serde_json::from_str::<InputEvent>(text.as_str())
                    {
                        let _ = incoming_tx.send((slot, event));
                    }
                }
            });

            while let Ok(snapshot_json) = outgoing_rx.recv().await {
                if write
                    .send(Message::Text(snapshot_json.into()))
                    .await
                    .is_err()
                {
                    break;
                }
            }

            reader.abort();

            // The connection is fully gone now - clean close, error, or the
            // peer process killed outright, all end up here the same way
            // (the write loop above breaks the moment a send fails, which
            // happens quickly once the peer is actually gone). Free its
            // slot for the next connection to claim, and tell the
            // simulation (tagged with the same slot) so it can stop
            // applying whatever movement this connection last held,
            // instead of a Character walking on forever with no one left
            // to release the key. Check whether the *other* slot is still
            // taken before releasing this one - that's the forfeit
            // decision (see `Disconnection`) - and check it under the same
            // lock acquisition as the release itself, so a concurrent
            // connect/disconnect on the other slot can't race between the
            // two.
            let opponent_connected = if let Some(player) = slot {
                let mut slots = slots.lock().expect("slot assignment mutex was poisoned");
                let opponent = other_player(player);
                let opponent_connected = slots.is_taken(opponent) && slots.is_settled(opponent);
                slots.set_taken(player, false);
                opponent_connected
            } else {
                false
            };
            let _ = disconnected_tx.send(Disconnection {
                slot,
                opponent_connected,
            });
        });
    }
}
