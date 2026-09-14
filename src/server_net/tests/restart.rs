use super::*;
use crate::combat::{PUNCH_DAMAGE, STARTING_HEALTH};
use crate::net_protocol::MatchStatus;

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
