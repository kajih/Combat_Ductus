//! Tests for the connection lifecycle this module owns: the `AppState`
//! machine (`Connecting` <-> `InMatch`), the `MyRole` reset that rides
//! along with a disconnect, and `ConnectionSlot::send_input`'s
//! do-nothing-without-a-connection contract. See
//! `docs/issues/combat-foundation/test-connect-screen-lifecycle.md`.
//!
//! Driven against a *real* local server (`spawn_network_thread` plus
//! `run_bevy_app`, from `server_net`), the way `client_net`'s and
//! `server_net`'s own tests already are, rather than against a faked
//! `Connection`: it holds an `ewebsock::WsSender`, which has no public
//! constructor, so mocking one would mean reshaping production code purely
//! to be mocked.
//! Testing against the real thing also makes the reconnect test mean
//! something - the slot the second connection gets is one the real slot
//! assignment really handed out, not one the test decided on.
//!
//! The Bevy half is the real `ConnectScreenPlugin` on a headless app
//! (`MinimalPlugins`, no rendering - the same shape the server's own Bevy
//! app uses), stepped one `update()` at a time so a test can watch the
//! state machine move rather than guess at when it did.
//!
//! The one thing a real server can't do on command is die -
//! `spawn_network_thread` hands back no shutdown handle, and its thread
//! runs until the process ends. `CuttableLink` fills that gap: a TCP relay
//! sitting between client and server that a test severs mid-Match, which
//! from the client's side is indistinguishable from the server process
//! being killed.

use super::*;
use bevy::state::app::StatesPlugin;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long any wait here tolerates before failing the test. Generous on
/// purpose: each one is waiting on a real TCP handshake and a real ~30Hz
/// server tick, and a loaded machine is a good deal slower than an idle
/// developer's.
const TIMEOUT: Duration = Duration::from_secs(10);

#[test]
fn a_dead_server_returns_to_the_connect_screen_and_clears_the_role() {
    let server = start_server();
    let link = CuttableLink::to(server);
    let mut app = headless_connect_screen_app();

    click_connect(&mut app, link.local_addr);
    update_until(&mut app, "the Match to start", |app| {
        current_state(app) == AppState::InMatch
    });
    // The very first connection is always assigned Player 1 - so there's a
    // real role here to be cleared by the disconnect below, not just an
    // `Unknown` that was never going to change.
    update_until(&mut app, "the server to assign a slot", |app| {
        my_role(app) == MyRole::Player(Player::P1)
    });

    link.cut();

    update_until(&mut app, "the Connect screen to come back", |app| {
        current_state(app) == AppState::Connecting
    });
    assert_eq!(
        my_role(&app),
        MyRole::Unknown,
        "a disconnect should clear the role, not leave the old one showing"
    );
    assert!(
        !is_connected(&app),
        "the dropped connection should be gone from the slot"
    );

    // The other half of `send_input`'s contract (the first half is
    // `sending_input_without_a_connection_is_silently_ignored` below):
    // input that arrives after the connection dropped is dropped too,
    // rather than panicking on the way out.
    app.world_mut()
        .non_send_mut::<ConnectionSlot>()
        .send_input(InputEvent::MoveRight(true));
}

#[test]
fn reconnecting_takes_the_slot_the_new_connection_is_actually_given() {
    let server = start_server();
    let link = CuttableLink::to(server);
    let mut app = headless_connect_screen_app();

    click_connect(&mut app, link.local_addr);
    update_until(&mut app, "the first connection to be given P1", |app| {
        my_role(app) == MyRole::Player(Player::P1)
    });

    link.cut();
    update_until(&mut app, "the Connect screen to come back", |app| {
        current_state(app) == AppState::Connecting
    });

    // Someone else takes over the slot that just came free, so
    // reconnecting can't quietly land back on P1 and pass this test by
    // accident.
    let _interloper = hold_slot(server, Player::P1);

    click_connect(&mut app, server);
    update_until(&mut app, "the Match to start again", |app| {
        current_state(app) == AppState::InMatch
    });
    update_until(&mut app, "the reconnection to be given a slot", |app| {
        my_role(app) != MyRole::Unknown
    });

    assert_eq!(
        my_role(&app),
        MyRole::Player(Player::P2),
        "the reconnection should show the slot it was actually given, not the one the dead connection held"
    );
}

#[test]
fn sending_input_without_a_connection_is_silently_ignored() {
    let mut slot = ConnectionSlot::default();

    // No connection has ever existed here - this is the state every client
    // sits in while the Connect screen is up, and the gameplay input module
    // pushes events through this same call every frame regardless.
    slot.send_input(InputEvent::MoveRight(true));
    slot.send_input(InputEvent::Punch);

    assert!(
        slot.0.is_none(),
        "sending input should never conjure a connection"
    );
}

/// Starts a real server on a free port, returning the address it bound to.
/// Its two halves (accept loop and simulation) run on their own threads for
/// the rest of the test process, the same way `client_net`'s tests start
/// one - there's no shutdown handle, and nothing that needs one.
fn start_server() -> SocketAddr {
    let parts = combat_ductus::server_net::spawn_network_thread("127.0.0.1:0")
        .expect("server should bind to a free port");
    let local_addr = parts.local_addr;
    std::thread::spawn(move || {
        combat_ductus::server_net::run_bevy_app(
            parts.incoming_rx,
            parts.outgoing_tx,
            parts.disconnected_rx,
        )
    });
    local_addr
}

/// The client under test, with nothing around it: no window, no renderer,
/// no other plugin's systems. `StatesPlugin` has to be added before
/// `ConnectScreenPlugin`, whose `init_state` panics without the
/// `StateTransition` schedule it registers (`DefaultPlugins` brings it
/// along, and with it everything this deliberately leaves out).
fn headless_connect_screen_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(StatesPlugin)
        .add_plugins(ConnectScreenPlugin);
    app
}

/// Types `address` into the Connect screen's address field and clicks
/// Connect - through the real widgets and the real observer, so the test
/// exercises the same path a player does rather than reaching into
/// `ConnectionSlot` itself.
fn click_connect(app: &mut App, address: SocketAddr) {
    // Let `OnEnter(Connecting)` spawn the screen first - on a fresh app
    // nothing exists yet, and after a disconnect the screen being clicked
    // is a newly spawned one.
    app.update();

    let world = app.world_mut();
    let mut inputs = world.query_filtered::<&mut EditableText, With<ServerAddressInput>>();
    let mut input = inputs
        .single_mut(world)
        .expect("the Connect screen has exactly one address field");
    *input = EditableText::new(address.to_string());

    let mut buttons = world.query_filtered::<Entity, With<WidgetButton>>();
    let button = buttons
        .single(world)
        .expect("the Connect screen has exactly one button");
    world.trigger(Activate { entity: button });
}

/// Steps the app until `done`, or fails the test naming what it was
/// waiting for. Everything these tests wait on is driven by another thread
/// (ewebsock's socket reader) or another process-worth of machinery (the
/// server's own tick), so there's nothing to do but keep stepping and
/// looking.
fn update_until(app: &mut App, what: &str, mut done: impl FnMut(&App) -> bool) {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        app.update();
        if done(app) {
            return;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn current_state(app: &App) -> AppState {
    *app.world().resource::<State<AppState>>().get()
}

fn my_role(app: &App) -> MyRole {
    *app.world().resource::<MyRole>()
}

fn is_connected(app: &App) -> bool {
    app.world().non_send::<ConnectionSlot>().0.is_some()
}

/// Opens a connection and keeps retrying until the server hands it
/// `wanted`, then leaves it open - the returned `Connection` holds that
/// slot for as long as the caller keeps it alive.
///
/// The retry is what makes this reliable: a slot isn't freed the instant
/// its client vanishes (the server only notices on its next broadcast
/// tick), so an attempt made immediately after a disconnect can still find
/// the old slot occupied and be handed the other one instead.
fn hold_slot(address: SocketAddr, wanted: Player) -> Connection {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let mut connection = Connection::connect(&format!("ws://{address}"))
            .expect("starting a connection attempt should succeed");
        if poll_for_slot(&mut connection) == Some(Some(wanted)) {
            return connection;
        }
        // Whatever slot this one did get, dropping it gives it straight
        // back, so the next attempt isn't competing with its predecessors.
        drop(connection);
        assert!(
            Instant::now() < deadline,
            "timed out waiting for the server to hand out {wanted:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Polls until the server's one-time `YourSlot` message arrives. `None`
/// means the connection ended, or went quiet long enough that it's not
/// worth waiting on - either way the caller should try again rather than
/// hang until the test's own deadline.
fn poll_for_slot(connection: &mut Connection) -> Option<Option<Player>> {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        match connection.poll() {
            Some(ConnectionEvent::YourSlot(slot)) => return Some(slot),
            Some(ConnectionEvent::Closed | ConnectionEvent::Error(_)) => return None,
            _ => std::thread::sleep(Duration::from_millis(5)),
        }
    }
    None
}

/// A TCP relay between a test's client and the real server, which the test
/// can sever on demand (`cut`) to stand in for the server process dying -
/// see this module's doc comment for why that needs standing in for.
///
/// Deliberately dumb: it copies bytes in both directions and understands
/// nothing about WebSocket frames, so cutting it drops the connection
/// mid-frame and unannounced, exactly as a killed process would.
struct CuttableLink {
    /// The address a client should connect to instead of the server's own.
    local_addr: SocketAddr,
    /// Both ends of every connection the relay is currently carrying, kept
    /// so `cut` can shut them down from the test's thread while the pump
    /// threads below are parked in a blocking read.
    live: Arc<Mutex<Vec<TcpStream>>>,
}

impl CuttableLink {
    fn to(upstream: SocketAddr) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("relay should bind to a free port");
        let local_addr = listener
            .local_addr()
            .expect("a bound listener has a local address");
        let live = Arc::new(Mutex::new(Vec::new()));

        let live_for_relay = Arc::clone(&live);
        std::thread::spawn(move || {
            for downstream in listener.incoming().flatten() {
                let Ok(upstream) = TcpStream::connect(upstream) else {
                    continue;
                };
                let (downstream_read, downstream_write) = split(downstream);
                let (upstream_read, upstream_write) = split(upstream);

                live_for_relay
                    .lock()
                    .expect("relay mutex was poisoned")
                    .extend([
                        downstream_read
                            .try_clone()
                            .expect("cloning a socket handle should succeed"),
                        upstream_read
                            .try_clone()
                            .expect("cloning a socket handle should succeed"),
                    ]);

                pump(downstream_read, upstream_write);
                pump(upstream_read, downstream_write);
            }
        });

        Self { local_addr, live }
    }

    /// Kills every connection the relay is carrying, in both directions at
    /// once: the client sees its server vanish, and the server sees its
    /// client vanish (which is what frees the slot again).
    fn cut(&self) {
        for stream in self
            .live
            .lock()
            .expect("relay mutex was poisoned")
            .drain(..)
        {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

/// Two handles to one socket, one per direction, so each direction can be
/// pumped by its own blocking thread.
fn split(stream: TcpStream) -> (TcpStream, TcpStream) {
    let clone = stream
        .try_clone()
        .expect("cloning a socket handle should succeed");
    (stream, clone)
}

/// Copies bytes one way until the source ends (or is cut out from under
/// it), then closes the destination so the peer at that end sees the
/// connection end too instead of hanging.
fn pump(mut from: TcpStream, mut to: TcpStream) {
    std::thread::spawn(move || {
        let _ = std::io::copy(&mut from, &mut to);
        let _ = to.shutdown(Shutdown::Both);
    });
}
