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
//! They talk to each other over a couple of plain channels: incoming
//! `InputEvent`s flow from the network thread into the Bevy world via a
//! `crossbeam_channel`, and outgoing `StateSnapshot` JSON flows out via a
//! `tokio::sync::broadcast` channel (which also gives spectator support -
//! a 3rd+ connection - for free later, since any number of receivers can
//! subscribe to the same broadcast).

use crate::combat::{MOVE_SPEED_PER_TICK, MatchState, Player};
use crate::net_protocol::{CharacterSnapshot, InputEvent, MatchStatus, StateSnapshot};
use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use crossbeam_channel::{Receiver as CbReceiver, Sender as CbSender};
use futures_util::{SinkExt, StreamExt};
use std::io;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

/// The simulation steps ~30 times per second.
const TICK_RATE_HZ: f64 = 30.0;

/// The pieces a caller needs to run the server: the address it actually
/// bound to (useful when binding to port 0), and the two channels
/// connecting network I/O to the simulation.
pub struct ServerParts {
    pub local_addr: SocketAddr,
    pub incoming_rx: CbReceiver<InputEvent>,
    pub outgoing_tx: broadcast::Sender<String>,
}

/// Bind a WebSocket listener at `bind_addr` and start accepting connections
/// on a background thread with its own tokio runtime. Returns as soon as
/// the listener is actually bound (so `bind_addr` can end in `:0` to let
/// the OS pick a free port, e.g. in tests).
pub fn spawn_network_thread(bind_addr: &str) -> io::Result<ServerParts> {
    let (incoming_tx, incoming_rx) = crossbeam_channel::unbounded::<InputEvent>();
    let (outgoing_tx, _) = broadcast::channel::<String>(32);
    let outgoing_tx_for_net = outgoing_tx.clone();
    let bind_addr = bind_addr.to_string();
    let (addr_tx, addr_rx) = std::sync::mpsc::channel::<io::Result<SocketAddr>>();

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

            accept_connections(listener, incoming_tx, outgoing_tx_for_net).await;
        });
    });

    let local_addr = addr_rx
        .recv()
        .map_err(|_| io::Error::other("server network thread exited before binding"))??;

    Ok(ServerParts {
        local_addr,
        incoming_rx,
        outgoing_tx,
    })
}

/// Accept WebSocket connections forever, forwarding each connection's
/// incoming input events into `incoming_tx` and forwarding every broadcast
/// snapshot out to that connection.
async fn accept_connections(
    listener: TcpListener,
    incoming_tx: CbSender<InputEvent>,
    outgoing: broadcast::Sender<String>,
) {
    loop {
        let (stream, _) = match listener.accept().await {
            Ok(pair) => pair,
            Err(_) => continue,
        };

        let incoming_tx = incoming_tx.clone();
        let mut outgoing_rx = outgoing.subscribe();

        tokio::spawn(async move {
            let ws_stream = match tokio_tungstenite::accept_async(stream).await {
                Ok(ws) => ws,
                Err(_) => return,
            };
            let (mut write, mut read) = ws_stream.split();

            let reader = tokio::spawn(async move {
                while let Some(Ok(message)) = read.next().await {
                    if let Message::Text(text) = message
                        && let Ok(event) = serde_json::from_str::<InputEvent>(text.as_str())
                    {
                        let _ = incoming_tx.send(event);
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
        });
    }
}

#[derive(Resource)]
struct MatchStateRes(MatchState);

#[derive(Resource)]
struct IncomingEvents(CbReceiver<InputEvent>);

#[derive(Resource, Default)]
struct HeldMovement {
    p1_left: bool,
    p1_right: bool,
}

#[derive(Resource)]
struct OutgoingSnapshots(broadcast::Sender<String>);

/// Run the headless simulation loop. Blocks forever (this is the server
/// binary's whole reason to exist), stepping `combat::MatchState` on a
/// fixed ~30Hz schedule regardless of whether a client is connected -
/// Player 2 exists as a stationary Idle Opponent from the very first tick.
pub fn run_bevy_app(incoming_rx: CbReceiver<InputEvent>, outgoing_tx: broadcast::Sender<String>) {
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
        .add_systems(Update, step_simulation)
        .run();
}

fn step_simulation(
    mut match_state: ResMut<MatchStateRes>,
    incoming: Res<IncomingEvents>,
    mut held: ResMut<HeldMovement>,
    outgoing: Res<OutgoingSnapshots>,
) {
    while let Ok(event) = incoming.0.try_recv() {
        match event {
            InputEvent::MoveLeft(pressed) => held.p1_left = pressed,
            InputEvent::MoveRight(pressed) => held.p1_right = pressed,
            InputEvent::Jump => match_state.0.jump(Player::P1),
            InputEvent::RequestRestart => {
                // Only honored once the Match has actually ended - this
                // resets a concluded Match, not an active one.
                if match_state.0.has_ended() {
                    match_state.0 = MatchState::new();
                }
            }
            other => {
                if let Some(attack) = other.as_attack() {
                    match_state.0.apply_attack(Player::P1, attack);
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
        speaking: character.speaking,
        attacking: character.attack_animation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{
        ATTACK_RANGE, KICK_DAMAGE, PUNCH_DAMAGE, SPECIAL_DAMAGE, STAGE_HALF_WIDTH, STARTING_HEALTH,
    };
    use crate::net_protocol::{InputEvent, MatchStatus};
    use std::time::Duration as StdDuration;

    #[tokio::test]
    async fn server_ticks_and_broadcasts_snapshots_before_any_client_connects() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

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
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

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
    async fn server_clamps_movement_at_the_stage_bound() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

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
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

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
    async fn jump_input_sends_player_one_airborne_and_they_land_again() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

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
    async fn special_input_lands_shows_the_speech_bubble_then_it_clears() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

        let url = format!("ws://{local_addr}");
        let (mut ws, _) = tokio_tungstenite::connect_async(url)
            .await
            .expect("client should be able to connect");

        // Starting positions (see MatchState::new) are already farther
        // apart than SPECIAL_MIN_RANGE, so Special can be cast right away
        // with no movement needed.
        send_event(&mut ws, InputEvent::Special).await;

        let cast = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if snapshot.p2.health < STARTING_HEALTH {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the special to land");
        assert_eq!(cast.p2.health, STARTING_HEALTH - SPECIAL_DAMAGE);
        assert!(cast.p1.speaking);

        let cleared = tokio::time::timeout(StdDuration::from_secs(5), async {
            loop {
                let snapshot = read_snapshot(&mut ws).await;
                if !snapshot.p1.speaking {
                    return snapshot;
                }
            }
        })
        .await
        .expect("timed out waiting for the speech bubble to clear");
        assert!(!cleared.p1.speaking);
    }

    #[tokio::test]
    async fn punch_lands_and_reduces_player_twos_health_when_in_range() {
        let ServerParts {
            local_addr,
            incoming_rx,
            outgoing_tx,
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

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
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

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
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

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
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

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
        } = spawn_network_thread("127.0.0.1:0").expect("server should bind to a free port");
        std::thread::spawn(move || run_bevy_app(incoming_rx, outgoing_tx));

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
