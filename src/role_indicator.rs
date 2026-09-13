//! Shows each connected client which role it holds - Player 1, Player 2,
//! or Spectating - persistently throughout the Match, so a client actually
//! has a way to tell. See
//! `docs/issues/combat-foundation/connection-identity-indicator.md`.

use crate::connect_screen::{AppState, MyRole};
use bevy::prelude::*;
use combat_ductus::combat::Player;

#[derive(Component)]
struct RoleIndicatorRoot;

#[derive(Component)]
struct RoleIndicatorText;

pub struct RoleIndicatorPlugin;

impl Plugin for RoleIndicatorPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), spawn_role_indicator)
            .add_systems(OnExit(AppState::InMatch), despawn_role_indicator)
            .add_systems(
                Update,
                update_role_indicator.run_if(in_state(AppState::InMatch)),
            );
    }
}

fn spawn_role_indicator(mut commands: Commands, my_role: Res<MyRole>) {
    commands
        .spawn((
            RoleIndicatorRoot,
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                position_type: PositionType::Absolute,
                bottom: Val::Px(16.0),
                left: Val::Px(0.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            // Same "dark panel behind white text" contrast approach the
            // Health HUD already established (see its own comment on why)
            // - guarantees legibility regardless of what's behind it on
            // the Stage.
            parent
                .spawn((
                    Node {
                        padding: UiRect::axes(Val::Px(10.0), Val::Px(4.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                ))
                .with_children(|parent| {
                    parent.spawn((
                        RoleIndicatorText,
                        Text::new(role_text(*my_role)),
                        TextColor(Color::WHITE),
                    ));
                });
        });
}

fn despawn_role_indicator(mut commands: Commands, roots: Query<Entity, With<RoleIndicatorRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

fn update_role_indicator(
    my_role: Res<MyRole>,
    mut texts: Query<&mut Text, With<RoleIndicatorText>>,
    mut last_shown: Local<Option<MyRole>>,
) {
    // MyRole only ever changes once per connection (Unknown -> Player/
    // Spectating, right after the server's one-time message arrives) -
    // this check keeps every other frame free, the same reasoning the
    // Health HUD's own update system already uses.
    if *last_shown == Some(*my_role) {
        return;
    }
    *last_shown = Some(*my_role);

    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    *text = Text::new(role_text(*my_role));
}

fn role_text(role: MyRole) -> &'static str {
    match role {
        // Briefly, before the server's one-time message arrives.
        MyRole::Unknown => "",
        MyRole::Player(Player::P1) => "Player 1",
        MyRole::Player(Player::P2) => "Player 2",
        MyRole::Spectating => "Spectating",
    }
}
