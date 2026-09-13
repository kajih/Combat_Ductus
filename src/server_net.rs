//! The headless `server` binary's guts: binds a WebSocket listener, steps
//! the authoritative `combat::MatchState` every tick regardless of whether
//! anyone is connected, applies a connected client's input, and broadcasts
//! a `net_protocol::StateSnapshot` to every connected client every tick.
//!
//! This module only exists on native targets (see `lib.rs`) - a browser tab
//! has no socket-listening API, so there is no wasm build to gate around
//! here, unlike most of this crate's other cfg-gating.
//!
//! Two halves run concurrently on separate OS threads:
//! - a small tokio runtime drives the WebSocket accept loop and per-client
//!   read/write tasks (`spawn_network_thread`)
//! - a headless Bevy `App` (`MinimalPlugins`, no rendering) steps the
//!   simulation on a fixed ~30Hz schedule (`run_bevy_app`)
//!
//! They talk to each other over a few plain channels: incoming
//! `InputEvent`s flow from the network thread into the Bevy world via a
//! `crossbeam_channel`, tagged with which player slot sent them (see
//! `SlotAssignment` below); outgoing `StateSnapshot` JSON flows out via a
//! `tokio::sync::broadcast` channel (which also gives spectator support -
//! a 3rd+ connection - for free, since any number of receivers can
//! subscribe to the same broadcast, and simply never gets a player slot
//! assigned - see `assign_slot`); and a connection ending (closed,
//! errored, or the peer process killed outright) signals the simulation
//! over a third `crossbeam_channel`, tagged with which slot (if any) that
//! connection held, so it can reset whatever movement that connection last
//! held - see `docs/issues/combat-foundation/reset-input-on-disconnect.md`
//! and `docs/issues/combat-foundation/second-client-controls-player-two.md`.

use crate::combat::{MOVE_SPEED_PER_TICK, MatchState, Player};
use crate::net_protocol::{CharacterSnapshot, InputEvent, MatchStatus, StateSnapshot};
use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use crossbeam_channel::{Receiver as CbReceiver, Sender as CbSender};
use futures_util::{SinkExt, StreamExt};
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

/// The simulation steps ~30 times per second.
const TICK_RATE_HZ: f64 = 30.0;

/// An input event tagged with which player slot sent it, if any. `None`
/// means the sending connection is a spectator (a 3rd+ connection - see
/// `assign_slot`) with no Character of its own to control; movement/attack
/// events from one of those are simply ignored, though `RequestRestart` is
/// still honored regardless of slot (see `step_simulation`), matching its
/// existing player-agnostic design from `restart-match.md`.
type TaggedInputEvent = (Option<Player>, InputEvent);

/// The pieces a caller needs to run the server: the address it actually
/// bound to (useful when binding to port 0), and the channels connecting
/// network I/O to the simulation.
pub struct ServerParts {
    pub local_addr: SocketAddr,
    pub incoming_rx: CbReceiver<TaggedInputEvent>,
    pub outgoing_tx: broadcast::Sender<String>,
    /// Fires once per connection, the moment that connection is fully gone
    /// (clean close, error, or an abrupt drop), carrying whichever player
    /// slot (if any) that connection held - see
    /// `docs/issues/combat-foundation/reset-input-on-disconnect.md`.
    pub disconnected_rx: CbReceiver<Option<Player>>,
}

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
}

impl SlotAssignment {
    fn is_taken(&self, player: Player) -> bool {
        match player {
            Player::P1 => self.p1_taken,
            Player::P2 => self.p2_taken,
        }
    }

    fn set_taken(&mut self, player: Player, taken: bool) {
        match player {
            Player::P1 => self.p1_taken = taken,
            Player::P2 => self.p2_taken = taken,
        }
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
    let (disconnected_tx, disconnected_rx) = crossbeam_channel::unbounded::<Option<Player>>();
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
    disconnected_tx: CbSender<Option<Player>>,
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
            // to release the key.
            if let Some(player) = slot {
                slots
                    .lock()
                    .expect("slot assignment mutex was poisoned")
                    .set_taken(player, false);
            }
            let _ = disconnected_tx.send(slot);
        });
    }
}

#[derive(Resource)]
struct MatchStateRes(MatchState);

#[derive(Resource)]
struct IncomingEvents(CbReceiver<TaggedInputEvent>);

/// Which movement keys are currently held, per player slot. Both Players
/// get their own pair now that a second real client can connect and
/// control Player 2 (see `second-client-controls-player-two.md`) - Player
/// 2's fields simply stay `false` for as long as it's an unclaimed Idle
/// Opponent, since nothing ever sends it movement input.
#[derive(Resource, Default)]
struct HeldMovement {
    p1_left: bool,
    p1_right: bool,
    p2_left: bool,
    p2_right: bool,
}

impl HeldMovement {
    fn set_left(&mut self, player: Player, pressed: bool) {
        match player {
            Player::P1 => self.p1_left = pressed,
            Player::P2 => self.p2_left = pressed,
        }
    }

    fn set_right(&mut self, player: Player, pressed: bool) {
        match player {
            Player::P1 => self.p1_right = pressed,
            Player::P2 => self.p2_right = pressed,
        }
    }

    /// Releases both of `player`'s held movement keys - used when their
    /// connection disconnects, so they stop walking on their own instead
    /// of continuing in whatever direction was last held.
    fn reset(&mut self, player: Player) {
        self.set_left(player, false);
        self.set_right(player, false);
    }
}

#[derive(Resource)]
struct OutgoingSnapshots(broadcast::Sender<String>);

#[derive(Resource)]
struct DisconnectedConnections(CbReceiver<Option<Player>>);

/// Run the headless simulation loop. Blocks forever (this is the server
/// binary's whole reason to exist), stepping `combat::MatchState` on a
/// fixed ~30Hz schedule regardless of whether a client is connected -
/// Player 2 exists as a stationary Idle Opponent from the very first tick.
pub fn run_bevy_app(
    incoming_rx: CbReceiver<TaggedInputEvent>,
    outgoing_tx: broadcast::Sender<String>,
    disconnected_rx: CbReceiver<Option<Player>>,
) {
    App::new()
        .add_plugins(
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / TICK_RATE_HZ,
            ))),
        )
        .insert_resource(MatchStateRes(MatchState::new()))
        .insert_resource(IncomingEvents(incoming_rx))
        .insert_resource(HeldMovement::default())
        .insert_resource(OutgoingSnapshots(outgoing_tx))
        .insert_resource(DisconnectedConnections(disconnected_rx))
        .add_systems(Update, step_simulation)
        .run();
}

fn step_simulation(
    mut match_state: ResMut<MatchStateRes>,
    incoming: Res<IncomingEvents>,
    mut held: ResMut<HeldMovement>,
    outgoing: Res<OutgoingSnapshots>,
    disconnected: Res<DisconnectedConnections>,
) {
    // A connection ending - reset whatever movement it left held, or that
    // Character keeps walking in that direction forever with no one left
    // to release the key. `None` (a spectator disconnecting) never held
    // any movement in the first place, so there's nothing to reset.
    // `try_recv` draining a channel that may have more than one queued
    // signal is harmless - the reset itself is idempotent.
    while let Ok(slot) = disconnected.0.try_recv() {
        if let Some(player) = slot {
            held.reset(player);
        }
    }

    while let Ok((slot, event)) = incoming.0.try_recv() {
        match event {
            // A spectator (no assigned slot) has no Character to move -
            // simply drop movement/attack input from one rather than
            // guessing which Character it meant.
            InputEvent::MoveLeft(pressed) => {
                if let Some(player) = slot {
                    held.set_left(player, pressed);
                }
            }
            InputEvent::MoveRight(pressed) => {
                if let Some(player) = slot {
                    held.set_right(player, pressed);
                }
            }
            InputEvent::Jump => {
                if let Some(player) = slot {
                    match_state.0.jump(player);
                }
            }
            InputEvent::RequestRestart => {
                // Not tied to a specific player - honored regardless of
                // slot (even from a spectator), same as before a second
                // slot existed. Only honored once the Match has actually
                // ended - this resets a concluded Match, not an active one.
                if match_state.0.has_ended() {
                    match_state.0 = MatchState::new();
                }
            }
            other => {
                if let (Some(player), Some(attack)) = (slot, other.as_attack()) {
                    match_state.0.apply_attack(player, attack);
                }
            }
        }
    }

    // move_player is itself a no-op once the Match has ended, and already
    // clamps to the Stage bounds - no need to guard has_ended() here too.
    if held.p1_left {
        match_state.0.move_player(Player::P1, -MOVE_SPEED_PER_TICK);
    }
    if held.p1_right {
        match_state.0.move_player(Player::P1, MOVE_SPEED_PER_TICK);
    }
    if held.p2_left {
        match_state.0.move_player(Player::P2, -MOVE_SPEED_PER_TICK);
    }
    if held.p2_right {
        match_state.0.move_player(Player::P2, MOVE_SPEED_PER_TICK);
    }

    match_state.0.advance_tick();

    let snapshot = snapshot_from_state(&match_state.0);
    if let Ok(json) = serde_json::to_string(&snapshot) {
        // No connected clients yet is a normal state (Idle Opponent era) -
        // an error here just means nobody is subscribed, not a real failure.
        let _ = outgoing.0.send(json);
    }
}

fn snapshot_from_state(state: &MatchState) -> StateSnapshot {
    StateSnapshot {
        tick: state.tick,
        p1: character_snapshot(&state.p1),
        p2: character_snapshot(&state.p2),
        status: match state.winner {
            Some(winner) => MatchStatus::Ended { winner },
            None => MatchStatus::InProgress,
        },
    }
}

fn character_snapshot(character: &crate::combat::CharacterState) -> CharacterSnapshot {
    CharacterSnapshot {
        position: character.position,
        facing: character.facing,
        health: character.health,
        airborne: character.airborne,
        vertical_offset: character.vertical_offset,
        attacking: character.attack_animation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{
        ATTACK_RANGE, KICK_DAMAGE, PUNCH_DAMAGE, STAGE_HALF_WIDTH, STARTING_HEALTH,
    };
    use crate::net_protocol::{InputEvent, MatchStatus};
    use std::time::Duration as StdDuration;

    #[tokio::test]
    async fn server_ticks_and_broadcasts_snapshots_before_any_client_connects() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        // The server steps and broadcasts on its own fixed schedule
        // regardless of input - the very first snapshot proves that.
        let message = tokio::time::timeout(StdDuration::from_secs(5), ws.next())
            .await
            .expect("timed out waiting for a snapshot")
            .expect("connection closed unexpectedly")
            .expect("websocket error");

        let Message::Text(text) = message else {
            panic!("expected a text snapshot message, got {message:?}");
        };
        let snapshot: StateSnapshot =
            serde_json::from_str(text.as_str()).expect("snapshot should deserialize");
        assert_eq!(snapshot.p1.health, STARTING_HEALTH);
        assert_eq!(snapshot.p2.health, STARTING_HEALTH);
        assert_eq!(snapshot.status, MatchStatus::InProgress);
    }

    #[tokio::test]
    async fn server_applies_a_connected_clients_input_to_player_one() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        let start_snapshot = read_snapshot(&mut ws).await;
        let starting_position = start_snapshot.p1.position;

        let event = InputEvent::MoveRight(true);
        ws.send(Message::Text(serde_json::to_string(&event).unwrap().into()))
            .await
            .expect("sending the input event should succeed");

        let moved_snapshot = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if snapshot.p1.position > starting_position {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the server to reflect the client's movement");

        assert!(moved_snapshot.p1.position > starting_position);
    }

    #[tokio::test]
    async fn a_second_connection_is_assigned_player_two_and_controls_it() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut p1, _) = tokio_tungstenite::connect_async(url.clone())
            .await
            .expect("first client should be able to connect");
        let (mut p2, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("second client should be able to connect");

        let starting_p2_position = read_snapshot(&mut p2).await.p2.position;

        // P2's connection moves - only P2's position should change.
        send_event(&mut p2, InputEvent::MoveLeft(true)).await;

        let moved = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut p2).await;
                if snapshot.p2.position < starting_p2_position {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the second connection's movement to apply to P2");

        assert!(moved.p2.position < starting_p2_position);
        // P1's connection never sent anything - its Character shouldn't
        // have moved as a side effect of P2's input.
        let p1_snapshot = read_snapshot(&mut p1).await;
        assert_eq!(p1_snapshot.p1.position, moved.p1.position);
    }

    #[tokio::test]
    async fn a_third_connection_is_a_spectator_and_controls_neither_player() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        // Take both real player slots first.
        let (_p1, _) = tokio_tungstenite::connect_async(url.clone())
            .await
            .expect("first client should be able to connect");
        let (_p2, _) = tokio_tungstenite::connect_async(url.clone())
            .await
            .expect("second client should be able to connect");
        let (mut spectator, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("third client should be able to connect");

        let starting = read_snapshot(&mut spectator).await;

        send_event(&mut spectator, InputEvent::MoveRight(true)).await;
        send_event(&mut spectator, InputEvent::MoveLeft(true)).await;
        send_event(&mut spectator, InputEvent::Punch).await;

        // Neither Character should move or lose Health from the
        // spectator's input across several ticks - it still receives
        // snapshots (proving it's connected at all), just never
        // influences the Match.
        for _ in 0..10 {
            let snapshot = read_snapshot(&mut spectator).await;
            assert_eq!(snapshot.p1.position, starting.p1.position);
            assert_eq!(snapshot.p2.position, starting.p2.position);
            assert_eq!(snapshot.p1.health, starting.p1.health);
            assert_eq!(snapshot.p2.health, starting.p2.health);
        }
    }

    #[tokio::test]
    async fn player_two_reverts_to_idle_when_its_connection_disconnects() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (_p1, _) = tokio_tungstenite::connect_async(url.clone())
            .await
            .expect("first client should be able to connect");
        let (mut p2, _) = tokio_tungstenite::connect_async(url.clone())
            .await
            .expect("second client should be able to connect");

        send_event(&mut p2, InputEvent::MoveLeft(true)).await;
        tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                if read_snapshot(&mut p2).await.p2.position < 0.0 {
                    return;
                }
            }
        })
        .await
        .expect("timed out waiting for P2 to start moving");

        // Abruptly drop P2's connection - same as a killed process.
        drop(p2);

        let (mut observer, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("observer client should be able to connect");

        let settled = tokio::time::timeout(StdDuration::from_secs(5), async {
            let mut previous = read_snapshot(&mut observer).await.p2.position;
            loop {
                let current = read_snapshot(&mut observer).await.p2.position;
                if current == previous {
                    return current;
                }
                previous = current;
            }
        })
        .await
        .expect("timed out waiting for P2's movement to stop after disconnecting");

        for _ in 0..10 {
            let snapshot = read_snapshot(&mut observer).await;
            assert_eq!(snapshot.p2.position, settled);
        }
    }

    #[tokio::test]
    async fn a_freed_player_slot_is_reassigned_to_the_next_connection() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (p1, _) = tokio_tungstenite::connect_async(url.clone())
            .await
            .expect("first client should be able to connect");

        // P1 disconnects before a second client ever shows up.
        drop(p1);

        // Give the server a moment to notice, then a fresh connection
        // should claim the now-free P1 slot (not P2 - P2 is still free
        // too, but P1 is offered first). Each retry attempt sends
        // MoveRight and checks whether P1's position actually rose above
        // wherever it started - if this connection instead landed as P2
        // or a spectator (because the old slot hadn't freed up yet),
        // dropping it at the end of the loop iteration and trying again
        // with a fresh connection is the only way to find out, since
        // nothing reports a connection's own assigned slot back to it.
        tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let Ok((mut ws, _)) = tokio_tungstenite::connect_async(url.clone()).await else {
                    continue;
                };
                let starting = read_snapshot(&mut ws).await.p1.position;
                send_event(&mut ws, InputEvent::MoveRight(true)).await;
                let moved = tokio::time::timeout(StdDuration::from_millis(500), async {
                    loop {
                        if read_snapshot(&mut ws).await.p1.position > starting {
                            return;
                        }
                    }
                })
                .await
                .is_ok();
                if moved {
                    return;
                }
            }
        })
        .await
        .expect("timed out waiting for a new connection to control P1");
    }

    #[tokio::test]
    async fn server_clamps_movement_at_the_stage_bound() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        let event = InputEvent::MoveRight(true);
        ws.send(Message::Text(serde_json::to_string(&event).unwrap().into()))
            .await
            .expect("send should succeed");

        // Held for far longer than it takes to cross the whole Stage -
        // position must saturate at the bound instead of drifting past it.
        let saturated = tokio::time::timeout(StdDuration::from_secs(10), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if snapshot.p1.position >= STAGE_HALF_WIDTH {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for movement to reach the stage bound");

        assert_eq!(saturated.p1.position, STAGE_HALF_WIDTH);

        // Give it several more ticks of continued held input - it must stay
        // exactly at the bound, never exceed it.
        for _ in 0..5 {
            let snapshot = read_snapshot(&mut ws).await;
            assert_eq!(snapshot.p1.position, STAGE_HALF_WIDTH);
        }
    }

    #[tokio::test]
    async fn releasing_movement_input_stops_the_position_from_changing() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        let start = InputEvent::MoveRight(true);
        ws.send(Message::Text(serde_json::to_string(&start).unwrap().into()))
            .await
            .expect("send should succeed");

        // Let it move for a bit, then release.
        tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                if read_snapshot(&mut ws).await.p1.position > 0.0 {
                    return;
                }
            }
        })
        .await
        .expect("timed out waiting for movement to start");

        let stop = InputEvent::MoveRight(false);
        ws.send(Message::Text(serde_json::to_string(&stop).unwrap().into()))
            .await
            .expect("send should succeed");

        // Find where it settles after release (allowing for the WS
        // round-trip, not asserting a single exact tick), then confirm it
        // holds there for several more ticks rather than continuing to
        // drift - i.e. movement actually stopped, not just paused.
        let settled = tokio::time::timeout(StdDuration::from_secs(5), async {
            let mut previous = read_snapshot(&mut ws).await.p1.position;
            loop {
                let current = read_snapshot(&mut ws).await.p1.position;
                if current == previous {
                    return current;
                }
                previous = current;
            }
        })
        .await
        .expect("timed out waiting for movement to stop changing");

        for _ in 0..5 {
            let snapshot = read_snapshot(&mut ws).await;
            assert_eq!(snapshot.p1.position, settled);
        }
    }

    #[tokio::test]
    async fn disconnecting_while_movement_is_held_stops_the_character_from_continuing_to_move() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url.clone())
            .await
            .expect("client should be able to connect");

        send_event(&mut ws, InputEvent::MoveRight(true)).await;

        tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                if read_snapshot(&mut ws).await.p1.position > 0.0 {
                    return;
                }
            }
        })
        .await
        .expect("timed out waiting for movement to start");

        // Abruptly drop the connection - no clean close/release event sent,
        // just gone, the same as a killed client process or a network
        // drop. The original connection can't be read from anymore once
        // it's dropped, so reconnect as a fresh observer to watch what
        // happens next.
        drop(ws);

        let (mut observer, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("observer client should be able to connect");

        // Find where the position settles (allowing for the server to
        // actually notice the drop), then confirm it holds there for
        // several more ticks rather than continuing to drift - i.e. the
        // held movement was actually reset, not just that the one
        // connection watching it went away.
        let settled = tokio::time::timeout(StdDuration::from_secs(5), async {
            let mut previous = read_snapshot(&mut observer).await.p1.position;
            loop {
                let current = read_snapshot(&mut observer).await.p1.position;
                if current == previous {
                    return current;
                }
                previous = current;
            }
        })
        .await
        .expect("timed out waiting for movement to stop changing after the disconnect");

        for _ in 0..10 {
            let snapshot = read_snapshot(&mut observer).await;
            assert_eq!(snapshot.p1.position, settled);
        }
    }

    #[tokio::test]
    async fn disconnecting_while_move_left_is_held_also_stops_the_character() {
        // The reset in step_simulation clears both held directions
        // unconditionally, but that's an implementation detail - confirm
        // the held-left case actually stops too, not just held-right.
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url.clone())
            .await
            .expect("client should be able to connect");

        send_event(&mut ws, InputEvent::MoveLeft(true)).await;

        tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                if read_snapshot(&mut ws).await.p1.position < 0.0 {
                    return;
                }
            }
        })
        .await
        .expect("timed out waiting for movement to start");

        drop(ws);

        let (mut observer, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("observer client should be able to connect");

        let settled = tokio::time::timeout(StdDuration::from_secs(5), async {
            let mut previous = read_snapshot(&mut observer).await.p1.position;
            loop {
                let current = read_snapshot(&mut observer).await.p1.position;
                if current == previous {
                    return current;
                }
                previous = current;
            }
        })
        .await
        .expect("timed out waiting for movement to stop changing after the disconnect");

        for _ in 0..10 {
            let snapshot = read_snapshot(&mut observer).await;
            assert_eq!(snapshot.p1.position, settled);
        }
    }

    #[tokio::test]
    async fn jump_input_sends_player_one_airborne_and_they_land_again() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        send_event(&mut ws, InputEvent::Jump).await;

        let airborne = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if snapshot.p1.airborne {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the jump to start");
        assert!(airborne.p1.vertical_offset > 0.0);

        let landed = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if !snapshot.p1.airborne {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the jump to end");
        assert_eq!(landed.p1.vertical_offset, 0.0);
    }

    #[tokio::test]
    async fn punch_lands_and_reduces_player_twos_health_when_in_range() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        move_p1_into_attack_range_of_p2(&mut ws).await;
        send_event(&mut ws, InputEvent::Punch).await;

        let hit = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if snapshot.p2.health < STARTING_HEALTH {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the punch to land");

        assert_eq!(hit.p2.health, STARTING_HEALTH - PUNCH_DAMAGE);
    }

    #[tokio::test]
    async fn kick_lands_and_reduces_player_twos_health_by_two_when_in_range() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        move_p1_into_attack_range_of_p2(&mut ws).await;
        send_event(&mut ws, InputEvent::Kick).await;

        let hit = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if snapshot.p2.health < STARTING_HEALTH {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the kick to land");

        assert_eq!(hit.p2.health, STARTING_HEALTH - KICK_DAMAGE);
    }

    #[tokio::test]
    async fn attacks_do_not_land_when_thrown_out_of_range() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        // P1 and P2 start far apart (see combat::MatchState::new) - well
        // out of Punch/Kick range - so throwing immediately should whiff.
        send_event(&mut ws, InputEvent::Punch).await;
        send_event(&mut ws, InputEvent::Kick).await;

        for _ in 0..10 {
            let snapshot = read_snapshot(&mut ws).await;
            assert_eq!(snapshot.p2.health, STARTING_HEALTH);
        }
    }

    #[tokio::test]
    async fn restart_request_is_ignored_while_the_match_is_in_progress() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        move_p1_into_attack_range_of_p2(&mut ws).await;
        send_event(&mut ws, InputEvent::Punch).await;

        let hit = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if snapshot.p2.health < STARTING_HEALTH {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the punch to land");
        assert_eq!(hit.p2.health, STARTING_HEALTH - PUNCH_DAMAGE);

        send_event(&mut ws, InputEvent::RequestRestart).await;

        // Give it several ticks to (incorrectly) reset if it were going
        // to - Health should stay right where the punch left it, and
        // status should never become anything but InProgress, since the
        // Match hasn't ended.
        for _ in 0..10 {
            let snapshot = read_snapshot(&mut ws).await;
            assert_eq!(snapshot.p2.health, STARTING_HEALTH - PUNCH_DAMAGE);
            assert_eq!(snapshot.status, MatchStatus::InProgress);
        }
    }

    #[tokio::test]
    async fn restart_resets_the_match_once_it_has_ended() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
            disconnected_rx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx, disconnected_rx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        move_p1_into_attack_range_of_p2(&mut ws).await;
        for _ in 0..STARTING_HEALTH {
            send_event(&mut ws, InputEvent::Punch).await;
        }

        let ended = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if matches!(snapshot.status, MatchStatus::Ended { .. }) {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the match to end");
        assert_eq!(ended.p2.health, 0);

        send_event(&mut ws, InputEvent::RequestRestart).await;

        let restarted = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if matches!(snapshot.status, MatchStatus::InProgress) {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the match to restart");

        assert_eq!(restarted.p1.health, STARTING_HEALTH);
        assert_eq!(restarted.p2.health, STARTING_HEALTH);
    }

    async fn send_event(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        event: InputEvent,
    ) {
        ws.send(Message::Text(serde_json::to_string(&event).unwrap().into()))
            .await
            .expect("send should succeed");
    }

    /// Sends P1 running toward P2 until they're within Punch/Kick range,
    /// then stops and waits for the position to settle - so a subsequent
    /// attack test isn't racing against P1 still drifting into or out of
    /// range from residual movement.
    async fn move_p1_into_attack_range_of_p2(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) {
        send_event(ws, InputEvent::MoveRight(true)).await;

        tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(ws).await;
                if (snapshot.p2.position - snapshot.p1.position).abs() <= ATTACK_RANGE {
                    return;
                }
            }
        })
        .await
        .expect("timed out waiting to get within attack range");

        send_event(ws, InputEvent::MoveRight(false)).await;

        tokio::time::timeout(StdDuration::from_secs(5), async {
            let mut previous = read_snapshot(ws).await.p1.position;
            loop {
                let current = read_snapshot(ws).await.p1.position;
                if current == previous {
                    return;
                }
                previous = current;
            }
        })
        .await
        .expect("timed out waiting for movement to settle");
    }

    async fn read_snapshot(
        ws: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> StateSnapshot {
        loop {
            let message = ws
                .next()
                .await
                .expect("connection closed unexpectedly")
                .expect("websocket error");
            if let Message::Text(text) = message {
                return serde_json::from_str(text.as_str()).expect("snapshot should deserialize");
            }
        }
    }
}
