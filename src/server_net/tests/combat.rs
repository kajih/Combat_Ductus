use super::*;
use crate::combat::{KICK_DAMAGE, PUNCH_DAMAGE, SPECIAL_DAMAGE, STARTING_HEALTH};

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
