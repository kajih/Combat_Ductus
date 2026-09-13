//! The Connect screen: lets the player type in the server's LAN address,
//! establishes the WebSocket connection, and transitions into the Match
//! once it succeeds. See
//! `docs/issues/combat-foundation/client-connect-and-lifecycle.md`.
//!
//! Rendering the actual Characters from the received state is a later issue
//! (`render-characters-from-server-state.md`) - this only has to get the
//! connection lifecycle right and keep the latest snapshot around
//! (`LatestSnapshot`) for that later work to read.

use bevy::prelude::*;
use bevy::text::EditableText;
use bevy::ui_widgets::{Activate, Button as WidgetButton};
use combat_ductus::client_net::{Connection, ConnectionEvent};
use combat_ductus::combat::Player;
use combat_ductus::net_protocol::{InputEvent, StateSnapshot};

const DEFAULT_SERVER_ADDRESS: &str = "127.0.0.1:9000";

/// The client's top-level screen/flow state.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Connecting,
    InMatch,
    /// The Match has ended (`net_protocol::MatchStatus::Ended`) - see
    /// `match_end_screen`, which is what actually transitions into and out
    /// of this state; this module only owns Connecting <-> InMatch.
    MatchEnded,
}

/// The most recent state snapshot received from the server, if any.
/// Later rendering work reads this - it doesn't need to know anything
/// about the connection itself.
#[derive(Resource, Default, Clone)]
pub struct LatestSnapshot(pub Option<StateSnapshot>);

/// This connection's own role - which player slot it controls, or that
/// it's spectating - once the server's one-time
/// `net_protocol::ServerMessage::YourSlot` has told it. See
/// `docs/issues/combat-foundation/connection-identity-indicator.md`.
/// Mirrors `LatestSnapshot`'s own pattern: other modules (e.g. the role
/// indicator) just read this resource, with no need to know anything
/// about the connection itself.
#[derive(Resource, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MyRole {
    /// Briefly, before the server's one-time message arrives.
    #[default]
    Unknown,
    Player(Player),
    Spectating,
}

#[derive(Component)]
struct ConnectScreenRoot;

#[derive(Component)]
struct ServerAddressInput;

#[derive(Component)]
struct StatusText;

/// Holds the connection once one has been started; `None` otherwise. Its
/// `Option` is the whole "are we connected" state - there's deliberately no
/// separate pending/active resource distinction.
///
/// `Connection` wraps platform-specific, non-thread-safe types (a
/// `std::sync::mpsc::Receiver` on native, an `Rc<web_sys::WebSocket>` on
/// wasm), so it can only ever be accessed from the main thread. That rules
/// out both an ordinary `Resource` (requires `Sync`) and moving it via
/// `Commands::queue` (requires the queued closure to be `Send`, and an `Rc`
/// never is). A pre-inserted *non-send* resource, mutated directly by
/// systems that declare `NonSendMut<ConnectionSlot>`, sidesteps both: access
/// is pinned to the main thread instead of requiring thread-safety.
///
/// `pub(crate)` (not private) so other client-only modules - e.g. the
/// gameplay input module - can send input events through the same
/// connection, via `send_input` below, without this module handing out the
/// underlying `Connection` (or the connect/disconnect logic) itself.
#[derive(Default)]
pub(crate) struct ConnectionSlot(Option<Connection>);

impl ConnectionSlot {
    /// Sends an input event if currently connected; silently does nothing
    /// otherwise (e.g. before a connection exists, or after it dropped).
    pub(crate) fn send_input(&mut self, event: InputEvent) {
        if let Some(connection) = self.0.as_mut() {
            connection.send_input(event);
        }
    }
}

pub struct ConnectScreenPlugin;

impl Plugin for ConnectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<LatestSnapshot>()
            .init_resource::<MyRole>()
            .insert_non_send(ConnectionSlot::default())
            .add_systems(OnEnter(AppState::Connecting), spawn_connect_screen)
            .add_systems(OnExit(AppState::Connecting), despawn_connect_screen)
            .add_systems(Update, poll_connection);
    }
}

fn spawn_connect_screen(mut commands: Commands) {
    commands
        .spawn((
            ConnectScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(12.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.08, 0.08, 0.10)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("Server address (host:port)"),
                TextColor(Color::WHITE),
            ));

            parent.spawn((
                ServerAddressInput,
                EditableText::new(DEFAULT_SERVER_ADDRESS),
                Node {
                    width: Val::Px(260.0),
                    height: Val::Px(36.0),
                    padding: UiRect::all(Val::Px(6.0)),
                    ..default()
                },
                BackgroundColor(Color::WHITE),
                TextColor(Color::BLACK),
            ));

            parent
                .spawn((
                    WidgetButton,
                    Node {
                        width: Val::Px(140.0),
                        height: Val::Px(36.0),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.2, 0.5, 0.9)),
                ))
                .with_children(|parent| {
                    parent.spawn((Text::new("Connect"), TextColor(Color::WHITE)));
                })
                .observe(on_connect_button_activated);

            parent.spawn((
                StatusText,
                Text::new(""),
                TextColor(Color::srgb(1.0, 0.65, 0.65)),
            ));
        });
}

fn despawn_connect_screen(mut commands: Commands, roots: Query<Entity, With<ConnectScreenRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

fn on_connect_button_activated(
    _activate: On<Activate>,
    input_query: Query<&EditableText, With<ServerAddressInput>>,
    mut status_query: Query<&mut Text, With<StatusText>>,
    mut slot: NonSendMut<ConnectionSlot>,
) {
    let Ok(editable_text) = input_query.single() else {
        return;
    };
    let address = editable_text.value().to_string();
    let address = address.trim();
    if address.is_empty() {
        set_status(&mut status_query, "Enter a server address first.");
        return;
    }

    let url = format!("ws://{address}");
    match Connection::connect(&url) {
        Ok(connection) => {
            slot.0 = Some(connection);
            set_status(&mut status_query, &format!("Connecting to {url}..."));
        }
        Err(err) => {
            set_status(&mut status_query, &format!("Could not connect: {err}"));
        }
    }
}

fn poll_connection(
    mut slot: NonSendMut<ConnectionSlot>,
    mut status_query: Query<&mut Text, With<StatusText>>,
    state: Res<State<AppState>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut latest_snapshot: ResMut<LatestSnapshot>,
    mut my_role: ResMut<MyRole>,
) {
    let Some(connection) = slot.0.as_mut() else {
        return;
    };

    let mut should_disconnect = false;

    while let Some(event) = connection.poll() {
        match event {
            ConnectionEvent::Opened => {
                info!("connected to server");
                set_status(&mut status_query, "Connected.");
                if *state.get() == AppState::Connecting {
                    next_state.set(AppState::InMatch);
                }
            }
            ConnectionEvent::YourSlot(slot) => {
                *my_role = match slot {
                    Some(player) => MyRole::Player(player),
                    None => MyRole::Spectating,
                };
            }
            ConnectionEvent::Snapshot(snapshot) => {
                // One of these arrives every server tick (~30/sec) - firehose
                // territory, not a notable event, so this is trace rather
                // than info (see docs/issues/observability/logging.md).
                trace!(tick = snapshot.tick, "received state snapshot");
                latest_snapshot.0 = Some(snapshot);
            }
            ConnectionEvent::Error(err) => {
                warn!(%err, "connection error");
                set_status(&mut status_query, &format!("Connection error: {err}"));
                should_disconnect = true;
            }
            ConnectionEvent::Closed => {
                warn!("connection closed");
                set_status(&mut status_query, "Connection closed.");
                should_disconnect = true;
            }
        }
    }

    if should_disconnect {
        slot.0 = None;
        // A new connection may be assigned a different slot entirely
        // (e.g. reconnecting after Player 1's real slot has already been
        // taken over by someone else) - don't leave a stale role showing
        // from this one.
        *my_role = MyRole::Unknown;
        // If we drop out of an in-progress Match (or the Match-Ended
        // screen), fall back to the Connect screen so the player isn't left
        // staring at a frozen, uninteractive view with no way to retry.
        // (The disconnect status text above won't actually be visible in
        // that case - the Connect screen doesn't exist yet this frame - but
        // the transition itself is what matters here.)
        if *state.get() != AppState::Connecting {
            next_state.set(AppState::Connecting);
        }
    }
}

fn set_status(status_query: &mut Query<&mut Text, With<StatusText>>, message: &str) {
    if let Ok(mut text) = status_query.single_mut() {
        *text = Text::new(message);
    }
}
