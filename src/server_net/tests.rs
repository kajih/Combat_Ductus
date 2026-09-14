//! Integration tests for `server_net`, driven over real WebSocket
//! connections against a real local server (`spawn_network_thread` +
//! `run_bevy_app`) rather than mocking any of it - see the parent module's
//! doc comment for the two halves under test.
//!
//! Split by what's under test, not by `server_net` API surface:
//! `connections` - slot assignment, spectator fallback, the `YourSlot`
//! message, freed-slot reassignment, a late joiner's first snapshot;
//! `disconnect` - Idle-reversion, forfeit (both the real thing and the
//! narrow race it doesn't trigger for), held-movement reset, reconnecting
//! to a forfeited slot; `movement` - stage-bound clamping and releasing
//! held movement; `combat` - Jump/Special/Punch/Kick landing and the
//! shared cooldown; `restart` - the restart control's gating, both by
//! Match status and by connection slot. All five are still
//! `crate::server_net::tests` itself, not separate top-level modules - see
//! the shared connection helpers below, used by all five via ordinary
//! private-to-ancestor visibility.

use super::*;
use crate::combat::ATTACK_RANGE;
use crate::net_protocol::{InputEvent, ServerMessage, StateSnapshot};
use futures_util::{SinkExt, StreamExt};
use std::time::Duration as StdDuration;
use tokio_tungstenite::tungstenite::Message;

mod combat;
mod connections;
mod disconnect;
mod movement;
mod restart;

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
