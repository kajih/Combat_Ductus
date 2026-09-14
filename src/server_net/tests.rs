//! Integration tests for `server_net`, driven over real WebSocket
//! connections against a real local server (`spawn_network_thread` +
//! `run_bevy_app`) rather than mocking any of it - see the parent module's
//! doc comment for the two halves under test.

use super::*;
use crate::combat::{
    ATTACK_RANGE, KICK_DAMAGE, MatchEndReason, PUNCH_DAMAGE, SPECIAL_DAMAGE, STAGE_HALF_WIDTH,
    STARTING_HEALTH,
};
use crate::net_protocol::{InputEvent, MatchStatus, ServerMessage, StateSnapshot};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration as StdDuration;
use tokio_tungstenite::tungstenite::Message;

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
    // (The very first *message* is actually this connection's own
    // YourSlot - read_snapshot skips past that on its way to the
    // first real Snapshot.)
    let snapshot = tokio::time::timeout(StdDuration::from_secs(5), read_snapshot(&mut ws))
        .await
        .expect("timed out waiting for a snapshot");
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
async fn each_connection_is_told_its_own_slot_right_after_connecting() {
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
    let (mut p2, _) = tokio_tungstenite::connect_async(url.clone())
        .await
        .expect("second client should be able to connect");
    let (mut spectator, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("third client should be able to connect");

    // The very first message each connection receives - before any
    // Snapshot - is its own YourSlot, not inferred from message order.
    assert_eq!(
        read_server_message(&mut p1).await,
        Some(ServerMessage::YourSlot(Some(Player::P1)))
    );
    assert_eq!(
        read_server_message(&mut p2).await,
        Some(ServerMessage::YourSlot(Some(Player::P2)))
    );
    assert_eq!(
        read_server_message(&mut spectator).await,
        Some(ServerMessage::YourSlot(None))
    );
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
async fn a_real_players_mid_match_disconnect_forfeits_the_match_to_the_remaining_player() {
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
    let (p2, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("second client should be able to connect");

    // Both slots are real connected clients now - confirm the Match is
    // actually under way, and give P1's slot a moment to genuinely
    // settle (see `MIN_OPPONENT_SETTLE`) before forfeiting it, the same
    // as any real Match would already be well past by the time anyone
    // disconnects.
    for _ in 0..10 {
        assert_eq!(read_snapshot(&mut p1).await.status, MatchStatus::InProgress);
    }

    // P2 vanishes mid-Match - same as a killed client process.
    drop(p2);

    let ended = tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            let snapshot = read_snapshot(&mut p1).await;
            if matches!(snapshot.status, MatchStatus::Ended { .. }) {
                return snapshot;
            }
        }
    })
    .await
    .expect("timed out waiting for the remaining player to see a forfeit win");

    assert_eq!(
        ended.status,
        MatchStatus::Ended {
            winner: Player::P1,
            reason: MatchEndReason::Forfeit,
        }
    );
}

#[tokio::test]
async fn disconnecting_while_the_other_slot_is_still_the_idle_opponent_does_not_end_the_match() {
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

    // P1 disconnects while P2 is still an unclaimed Idle Opponent - no
    // real opponent to award a win to, so this is a no-op for the
    // Match, exactly like today (see `player_two_reverts_to_idle...`'s
    // sibling case, and `a_freed_player_slot_is_reassigned...` for the
    // slot itself becoming available again).
    drop(p1);

    let (mut observer, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("observer client should be able to connect");

    for _ in 0..10 {
        let snapshot = read_snapshot(&mut observer).await;
        assert_eq!(snapshot.status, MatchStatus::InProgress);
    }
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
async fn special_input_lands_shows_the_speech_bubble_then_it_clears() {
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
async fn rapid_repeated_punch_and_kick_presses_are_throttled_by_the_shared_cooldown() {
    // See punch-kick-cooldown.md: mashing J/K should land at Punch's
    // steady cadence, not deal damage on every single keypress.
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

    // Mash J and K back-to-back, far faster than the cooldown allows -
    // only the very first should land.
    for _ in 0..STARTING_HEALTH {
        send_event(&mut ws, InputEvent::Punch).await;
        send_event(&mut ws, InputEvent::Kick).await;
    }

    let after_burst = tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            let snapshot = read_snapshot(&mut ws).await;
            if snapshot.p2.health < STARTING_HEALTH {
                return snapshot;
            }
        }
    })
    .await
    .expect("timed out waiting for the first attack in the burst to land");
    assert_eq!(after_burst.p2.health, STARTING_HEALTH - PUNCH_DAMAGE);

    // Give the server plenty of ticks to (incorrectly) process the rest
    // of the burst if it were going to - Health should hold right
    // where the first hit left it.
    for _ in 0..10 {
        let snapshot = read_snapshot(&mut ws).await;
        assert_eq!(snapshot.p2.health, STARTING_HEALTH - PUNCH_DAMAGE);
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
    // Punch is now cooldown-gated (punch-kick-cooldown.md), so a single
    // rapid-fire attempt can land on the wrong side of it and be
    // silently dropped - throw_p1_attack_until retries until each one
    // actually registers before moving on to the next.
    let mut expected_health = STARTING_HEALTH;
    for _ in 0..STARTING_HEALTH {
        expected_health -= PUNCH_DAMAGE;
        throw_p1_attack_until(&mut ws, InputEvent::Punch, |snapshot| {
            snapshot.p2.health <= expected_health
        })
        .await;
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

#[tokio::test]
async fn restart_resets_held_movement_so_a_character_does_not_keep_walking_on_its_own() {
    // Reproduces reset-input-on-match-restart.md: a player holding a
    // movement key at the instant the winning blow lands never gets a
    // chance to send the release event (the client leaves
    // AppState::InMatch and stops running its input systems entirely) -
    // simulated here by simply never sending MoveRight(false) at all.
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

    // Whittle P2's Health down to the very last hit point without
    // holding any movement, so the cooldown-spaced Punches (see
    // punch-kick-cooldown.md) don't have time to drift P1 out of range.
    let mut expected_health = STARTING_HEALTH;
    for _ in 0..STARTING_HEALTH - PUNCH_DAMAGE {
        expected_health -= PUNCH_DAMAGE;
        throw_p1_attack_until(&mut ws, InputEvent::Punch, |snapshot| {
            snapshot.p2.health <= expected_health
        })
        .await;
    }

    // Now start holding movement right at the instant of the winning
    // blow itself - the release event a real client would send once
    // the Match-Ended screen is showing is deliberately never sent.
    send_event(&mut ws, InputEvent::MoveRight(true)).await;
    throw_p1_attack_until(&mut ws, InputEvent::Punch, |snapshot| {
        snapshot.p2.health == 0
    })
    .await;

    tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            let snapshot = read_snapshot(&mut ws).await;
            if matches!(snapshot.status, MatchStatus::Ended { .. }) {
                return snapshot;
            }
        }
    })
    .await
    .expect("timed out waiting for the match to end");

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

    // MoveRight(true) was never released - if HeldMovement weren't
    // reset on restart, P1 would immediately resume drifting right on
    // its own from the new Match's very first tick.
    let starting_position = restarted.p1.position;
    for _ in 0..10 {
        let snapshot = read_snapshot(&mut ws).await;
        assert_eq!(snapshot.p1.position, starting_position);
    }
}

#[tokio::test]
async fn a_spectators_restart_request_has_no_effect_but_a_players_still_works() {
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
    let (_p2, _) = tokio_tungstenite::connect_async(url.clone())
        .await
        .expect("second client should be able to connect");

    move_p1_into_attack_range_of_p2(&mut p1).await;
    let mut expected_health = STARTING_HEALTH;
    for _ in 0..STARTING_HEALTH {
        expected_health -= PUNCH_DAMAGE;
        throw_p1_attack_until(&mut p1, InputEvent::Punch, |snapshot| {
            snapshot.p2.health <= expected_health
        })
        .await;
    }
    tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            if matches!(
                read_snapshot(&mut p1).await.status,
                MatchStatus::Ended { .. }
            ) {
                return;
            }
        }
    })
    .await
    .expect("timed out waiting for the match to end");

    // Both player slots are already taken, so this connects as a
    // spectator - joining only now (after the Match already ended)
    // means its own snapshot stream starts clean, with no backlog of
    // earlier in-progress frames to wade through first.
    let (mut spectator, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("spectator client should be able to connect");
    assert!(matches!(
        read_snapshot(&mut spectator).await.status,
        MatchStatus::Ended { .. }
    ));

    // A spectator has no player slot - its RequestRestart should be
    // silently ignored, leaving the Match ended.
    send_event(&mut spectator, InputEvent::RequestRestart).await;
    for _ in 0..10 {
        let snapshot = read_snapshot(&mut spectator).await;
        assert!(matches!(snapshot.status, MatchStatus::Ended { .. }));
    }

    // A real player's RequestRestart still works, exactly as before.
    send_event(&mut p1, InputEvent::RequestRestart).await;
    let restarted = tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            let snapshot = read_snapshot(&mut spectator).await;
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

#[tokio::test]
async fn a_connection_joining_after_the_match_has_ended_sees_ended_status_immediately() {
    // See docs/adr/0009-reconnecting-inherits-current-match-state.md -
    // a late connection gets the truthful current status right away,
    // not a stale InProgress snapshot first.
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

    move_p1_into_attack_range_of_p2(&mut p1).await;
    // Punch is cooldown-gated (punch-kick-cooldown.md) - land one hit
    // point at a time rather than firing a burst that would mostly get
    // silently dropped.
    let mut expected_health = STARTING_HEALTH;
    for _ in 0..STARTING_HEALTH {
        expected_health -= PUNCH_DAMAGE;
        throw_p1_attack_until(&mut p1, InputEvent::Punch, |snapshot| {
            snapshot.p2.health <= expected_health
        })
        .await;
    }
    tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            if matches!(
                read_snapshot(&mut p1).await.status,
                MatchStatus::Ended { .. }
            ) {
                return;
            }
        }
    })
    .await
    .expect("timed out waiting for the match to end");

    // Connect fresh, after the match is already over - its very first
    // snapshot should already report Ended, not InProgress.
    let (mut latecomer, _) = tokio_tungstenite::connect_async(url)
        .await
        .expect("late client should be able to connect");
    let first_snapshot = read_snapshot(&mut latecomer).await;
    assert!(matches!(first_snapshot.status, MatchStatus::Ended { .. }));
}

#[tokio::test]
async fn a_connection_taking_over_a_forfeited_slot_inherits_the_ended_match_not_a_fresh_start() {
    // See docs/adr/0009-reconnecting-inherits-current-match-state.md -
    // no reset-on-reconnect, or a losing player could escape a bad
    // position (or, since forfeit-win-on-disconnect.md, a forfeited
    // Match) just by disconnecting and reconnecting. P2 disconnecting
    // from an ongoing real-vs-real Match is exactly what forfeits it
    // now (a_real_players_mid_match_disconnect_forfeits...), so this is
    // the scenario ADR 0009 itself anticipates: a connection joining
    // after the Match has already ended sees the truthful Ended status
    // and the Health it actually ended with, not a reset.
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
    let (p2, _) = tokio_tungstenite::connect_async(url.clone())
        .await
        .expect("second client should be able to connect");

    // Land one Punch on P2 (not a killing blow - Health should freeze
    // right here once the forfeit below ends the Match, not reach 0
    // some other way), then give P1's slot time to settle
    // (`MIN_OPPONENT_SETTLE`) before P2 disconnects.
    move_p1_into_attack_range_of_p2(&mut p1).await;
    send_event(&mut p1, InputEvent::Punch).await;
    let damaged = tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            let snapshot = read_snapshot(&mut p1).await;
            if snapshot.p2.health < STARTING_HEALTH {
                return snapshot;
            }
        }
    })
    .await
    .expect("timed out waiting for the punch to land");
    assert_eq!(damaged.p2.health, STARTING_HEALTH - PUNCH_DAMAGE);
    for _ in 0..10 {
        read_snapshot(&mut p1).await;
    }

    drop(p2);

    tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            if matches!(
                read_snapshot(&mut p1).await.status,
                MatchStatus::Ended { .. }
            ) {
                return;
            }
        }
    })
    .await
    .expect("timed out waiting for P2's disconnect to forfeit the match");

    // A fresh connection claims the now-free P2 slot - confirmed by its
    // own one-time YourSlot message, since P2 can no longer prove
    // control by moving (the Match has already ended, so movement is a
    // no-op regardless of who sends it). Retry (as in
    // a_freed_player_slot_is_reassigned_to_the_next_connection) since
    // the slot may not have freed up by the very first attempt.
    let inherited = tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            let Ok((mut ws, _)) = tokio_tungstenite::connect_async(url.clone()).await else {
                continue;
            };
            if read_server_message(&mut ws).await == Some(ServerMessage::YourSlot(Some(Player::P2)))
            {
                return read_snapshot(&mut ws).await;
            }
        }
    })
    .await
    .expect("timed out waiting for a new connection to control the vacated P2 slot");

    assert_eq!(inherited.p2.health, STARTING_HEALTH - PUNCH_DAMAGE);
    assert_eq!(
        inherited.status,
        MatchStatus::Ended {
            winner: Player::P1,
            reason: MatchEndReason::Forfeit,
        }
    );
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

/// Throws P1's `attack` (Punch or Kick) repeatedly until `is_landed`
/// reports true against a snapshot, resending every few ticks rather
/// than once - a lone attempt can land on the wrong side of Punch/Kick's
/// cooldown (punch-kick-cooldown.md) and be silently dropped with no
/// feedback, so this is what every test that needs a hit to definitely
/// register should use instead of a single `send_event`.
async fn throw_p1_attack_until(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    attack: InputEvent,
    is_landed: impl Fn(&StateSnapshot) -> bool,
) -> StateSnapshot {
    tokio::time::timeout(StdDuration::from_secs(5), async {
        loop {
            send_event(ws, attack).await;
            for _ in 0..5 {
                let snapshot = read_snapshot(ws).await;
                if is_landed(&snapshot) {
                    return snapshot;
                }
            }
        }
    })
    .await
    .expect("timed out waiting for the attack to land")
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

/// Reads server messages until a `Snapshot` arrives, silently skipping
/// anything else (namely the one-time `YourSlot` message every
/// connection gets right after connecting - most tests here don't care
/// about their own slot, only the ongoing snapshot stream).
async fn read_snapshot(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> StateSnapshot {
    loop {
        if let Some(ServerMessage::Snapshot(snapshot)) = read_server_message(ws).await {
            return snapshot;
        }
    }
}

/// Reads the next server message and deserializes it as a
/// `ServerMessage` - `None` if the frame wasn't text (e.g. a
/// ping/pong), so callers can loop past those without treating them as
/// a protocol violation.
async fn read_server_message(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> Option<ServerMessage> {
    let message = ws
        .next()
        .await
        .expect("connection closed unexpectedly")
        .expect("websocket error");
    match message {
        Message::Text(text) => {
            Some(serde_json::from_str(text.as_str()).expect("message should deserialize"))
        }
        _ => None,
    }
}
