//! A minimal Health readout for both Characters - plain text, no HUD
//! polish, per this issue's own scope. See
//! `docs/issues/combat-foundation/punch-kick-health-depletion.md`.

use crate::connect_screen::{AppState, LatestSnapshot};
use bevy::prelude::*;

#[derive(Component)]
struct HealthHudRoot;

#[derive(Component, Clone, Copy)]
enum HealthLabel {
    P1,
    P2,
}

impl HealthLabel {
    fn prefix(self) -> &'static str {
        match self {
            HealthLabel::P1 => "P1",
            HealthLabel::P2 => "P2",
        }
    }
}

pub struct HealthHudPlugin;

impl Plugin for HealthHudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), spawn_health_hud)
            .add_systems(OnExit(AppState::InMatch), despawn_health_hud)
            .add_systems(
                Update,
                update_health_hud.run_if(in_state(AppState::InMatch)),
            );
    }
}

fn spawn_health_hud(mut commands: Commands) {
    commands
        .spawn((
            HealthHudRoot,
            Node {
                width: Val::Percent(100.0),
                justify_content: JustifyContent::SpaceBetween,
                padding: UiRect::all(Val::Px(16.0)),
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                ..default()
            },
        ))
        .with_children(|parent| {
            for label in [HealthLabel::P1, HealthLabel::P2] {
                // The Stage has both a light wall band and a darker floor
                // band behind the HUD depending on window size/aspect, so
                // plain text in either color can end up nearly invisible
                // against one of them. A dark panel behind white text
                // guarantees contrast regardless of what's behind it.
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
                            label,
                            Text::new(health_text(label, combat_ductus::combat::STARTING_HEALTH)),
                            TextColor(Color::WHITE),
                        ));
                    });
            }
        });
}

fn despawn_health_hud(mut commands: Commands, roots: Query<Entity, With<HealthHudRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

fn update_health_hud(
    latest_snapshot: Res<LatestSnapshot>,
    mut labels: Query<(&HealthLabel, &mut Text)>,
    mut last_shown: Local<Option<(u8, u8)>>,
) {
    let Some(snapshot) = &latest_snapshot.0 else {
        return;
    };

    // LatestSnapshot is overwritten every server tick (~30/sec) regardless
    // of whether Health actually changed, so without this check we'd
    // reassign Text - forcing a full text-layout pass - every single frame
    // even while nothing is happening. Health only ever changes on a
    // landed hit, so this keeps the common case free.
    let current = (snapshot.p1.health, snapshot.p2.health);
    if *last_shown == Some(current) {
        return;
    }
    *last_shown = Some(current);

    for (label, mut text) in &mut labels {
        let health = match label {
            HealthLabel::P1 => current.0,
            HealthLabel::P2 => current.1,
        };
        *text = Text::new(health_text(*label, health));
    }
}

fn health_text(label: HealthLabel, health: u8) -> String {
    format!("{}: {health}", label.prefix())
}
