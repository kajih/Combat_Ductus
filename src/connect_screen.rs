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
use combat_ductus::net_protocol::StateSnapshot;

const DEFAULT_SERVER_ADDRESS: &str = "127.0.0.1:9000";

/// The client's top-level screen/flow state.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppState {
    #[default]
    Connecting,
    InMatch,
}

/// The most recent state snapshot received from the server, if any.
/// Later rendering work reads this - it doesn't need to know anything
/// about the connection itself.
#[derive(Resource, Default, Clone)]
pub struct LatestSnapshot(pub Option<StateSnapshot>);

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
#[derive(Default)]
struct ConnectionSlot(Option<Connection>);

pub struct ConnectScreenPlugin;

impl Plugin for ConnectScreenPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<AppState>()
            .init_resource::<LatestSnapshot>()
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
            ConnectionEvent::Snapshot(snapshot) => {
                info!(tick = snapshot.tick, "received state snapshot");
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
        // If we drop out of an in-progress Match, fall back to the Connect
        // screen so the player isn't left staring at a frozen game with no
        // way to retry. (The disconnect status text above won't actually be
        // visible in that case - the Connect screen doesn't exist yet this
        // frame - but the transition itself is what matters here.)
        if *state.get() == AppState::InMatch {
            next_state.set(AppState::Connecting);
        }
    }
}

fn set_status(status_query: &mut Query<&mut Text, With<StatusText>>, message: &str) {
    if let Ok(mut text) = status_query.single_mut() {
        *text = Text::new(message);
    }
}
