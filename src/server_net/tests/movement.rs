use super::*;
use crate::combat::STAGE_HALF_WIDTH;

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
