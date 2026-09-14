use super::*;
use crate::combat::{PUNCH_DAMAGE, STARTING_HEALTH};
use crate::net_protocol::MatchStatus;

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
