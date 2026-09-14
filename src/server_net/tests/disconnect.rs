use super::*;
use crate::combat::{MatchEndReason, PUNCH_DAMAGE, STARTING_HEALTH};
use crate::net_protocol::MatchStatus;

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
