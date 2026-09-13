//! The client's WebSocket connection to the `server` binary.
//!
//! Uses `ewebsock`, which compiles to both native and `wasm32-unknown-unknown`
//! with the same poll-based API, so this module needs no `cfg`-gating of its
//! own (unlike `server_net`, which only exists on native).

use crate::combat::Player;
use crate::net_protocol::{InputEvent, ServerMessage, StateSnapshot};
use ewebsock::{WsEvent, WsMessage};

/// Something worth telling the rest of the client about, distilled from the
/// raw `ewebsock` events into terms this crate's callers care about.
#[derive(Debug, Clone)]
pub enum ConnectionEvent {
    /// The WebSocket handshake completed; the connection is now usable.
    Opened,
    /// This connection's own role - arrives exactly once, from the
    /// server's one-time `net_protocol::ServerMessage::YourSlot`.
    YourSlot(Option<Player>),
    /// A `net_protocol::StateSnapshot` arrived and deserialized successfully.
    Snapshot(StateSnapshot),
    /// The connection failed, or a message arrived that wasn't a server
    /// message this client understood.
    Error(String),
    /// The connection was closed (by either side).
    Closed,
}

/// A connection to the server. Poll it every frame (see `poll`) - it never
/// blocks.
pub struct Connection {
    sender: ewebsock::WsSender,
    receiver: ewebsock::WsReceiver,
}

impl Connection {
    /// Start connecting to `url` (e.g. `"ws://127.0.0.1:9000"`). Returns as
    /// soon as the connection attempt has started - use `poll` to find out
    /// whether it actually succeeds.
    pub fn connect(url: &str) -> Result<Self, String> {
        let (sender, receiver) = ewebsock::connect(url, ewebsock::Options::default())?;
        Ok(Connection { sender, receiver })
    }

    /// Send an input event to the server. Silently drops it if it somehow
    /// fails to serialize (it never should, `InputEvent` is a plain enum).
    pub fn send_input(&mut self, event: InputEvent) {
        if let Ok(json) = serde_json::to_string(&event) {
            self.sender.send(WsMessage::Text(json));
        }
    }

    /// Non-blocking: returns the next connection event, if any arrived since
    /// the last call. Call this in a loop (draining it) once per frame.
    pub fn poll(&mut self) -> Option<ConnectionEvent> {
        match self.receiver.try_recv()? {
            WsEvent::Opened => Some(ConnectionEvent::Opened),
            WsEvent::Message(WsMessage::Text(text)) => match serde_json::from_str(&text) {
                Ok(ServerMessage::YourSlot(slot)) => Some(ConnectionEvent::YourSlot(slot)),
                Ok(ServerMessage::Snapshot(snapshot)) => Some(ConnectionEvent::Snapshot(snapshot)),
                Err(err) => Some(ConnectionEvent::Error(format!(
                    "received a message that wasn't a valid server message: {err}"
                ))),
            },
            // Binary/ping/pong/unknown frames aren't part of this protocol.
            WsEvent::Message(_) => None,
            WsEvent::Error(err) => Some(ConnectionEvent::Error(err)),
            WsEvent::Closed => Some(ConnectionEvent::Closed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::STARTING_HEALTH;
    use crate::net_protocol::InputEvent;
    use std::time::{Duration, Instant};

    /// Poll `connection` until `f` returns `Some`, or panic after `timeout`.
    /// `Connection::poll` is non-blocking by design (it has to be, since it
    /// also gets called from a Bevy system every frame), so driving it from
    /// a plain test needs this kind of small spin-loop instead of `.await`.
    fn poll_until<T>(
        connection: &mut Connection,
        timeout: Duration,
        mut f: impl FnMut(ConnectionEvent) -> Option<T>,
    ) -> T {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(event) = connection.poll()
                && let Some(value) = f(event)
            {
                return value;
            }
            assert!(Instant::now() < deadline, "timed out waiting for event");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn connects_to_the_real_server_and_receives_snapshots() {
        let parts = crate::server_net::spawn_network_thread("127.0.0.1:0")
            .expect("server should bind to a free port");
        std::thread::spawn(move || {
            crate::server_net::run_bevy_app(
                parts.incoming_rx,
                parts.outgoing_tx,
                parts.disconnected_rx,
            )
        });

        let mut connection = Connection::connect(&format!("ws://{}", parts.local_addr))
            .expect("starting a connection attempt should succeed");

        poll_until(&mut connection, Duration::from_secs(5), |event| {
            matches!(event, ConnectionEvent::Opened).then_some(())
        });

        // The very first connection is always assigned Player 1.
        let slot = poll_until(
            &mut connection,
            Duration::from_secs(5),
            |event| match event {
                ConnectionEvent::YourSlot(slot) => Some(slot),
                _ => None,
            },
        );
        assert_eq!(slot, Some(Player::P1));

        let snapshot = poll_until(
            &mut connection,
            Duration::from_secs(5),
            |event| match event {
                ConnectionEvent::Snapshot(snapshot) => Some(snapshot),
                _ => None,
            },
        );
        assert_eq!(snapshot.p1.health, STARTING_HEALTH);

        // Sending an input should be reflected in a later snapshot, proving
        // this isn't just a one-way read-only connection.
        connection.send_input(InputEvent::MoveRight(true));
        let moved = poll_until(
            &mut connection,
            Duration::from_secs(5),
            |event| match event {
                ConnectionEvent::Snapshot(snapshot) if snapshot.p1.position > 0.0 => Some(snapshot),
                _ => None,
            },
        );
        assert!(moved.p1.position > 0.0);
    }

    #[test]
    fn reports_an_error_for_a_connection_that_is_refused() {
        // Bind to let the OS hand us a free port, then immediately drop the
        // listener so nothing is actually listening there any more - the
        // connection attempt below should be refused rather than silently
        // hang forever.
        let local_addr = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("should bind");
            listener
                .local_addr()
                .expect("a bound listener has a local address")
        };

        let mut connection = Connection::connect(&format!("ws://{local_addr}"))
            .expect("starting a connection attempt should succeed even if it later fails");

        poll_until(&mut connection, Duration::from_secs(5), |event| {
            matches!(event, ConnectionEvent::Error(_) | ConnectionEvent::Closed).then_some(())
        });
    }
}
