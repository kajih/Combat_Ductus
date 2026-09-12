//! Shows a static "X wins" screen once the Match ends. See
//! `docs/issues/combat-foundation/match-end-screen.md`.
//!
//! The server already stops applying further damage/movement the instant
//! the Match ends (`combat::MatchState::apply_attack`/`move_player` both
//! no-op via `has_ended()`) and already broadcasts who won
//! (`net_protocol::MatchStatus::Ended`) - this module's whole job is
//! reacting to that on the client: notice it, leave In-Match, and show the
//! winner. No restart control lives here - that's
//! `docs/issues/combat-foundation/restart-match.md`, built on top of this
//! screen rather than folded into it.

use crate::connect_screen::{AppState, LatestSnapshot};
use bevy::prelude::*;
use combat_ductus::combat::Player;
use combat_ductus::net_protocol::MatchStatus;

#[derive(Component)]
struct MatchEndScreenRoot;

pub struct MatchEndScreenPlugin;

impl Plugin for MatchEndScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, detect_match_end.run_if(in_state(AppState::InMatch)))
            .add_systems(OnEnter(AppState::MatchEnded), spawn_match_end_screen)
            .add_systems(OnExit(AppState::MatchEnded), despawn_match_end_screen);
    }
}

fn detect_match_end(
    latest_snapshot: Res<LatestSnapshot>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    let Some(snapshot) = &latest_snapshot.0 else {
        return;
    };

    if matches!(snapshot.status, MatchStatus::Ended { .. }) {
        next_state.set(AppState::MatchEnded);
    }
}

fn spawn_match_end_screen(mut commands: Commands, latest_snapshot: Res<LatestSnapshot>) {
    let winner = latest_snapshot.0.as_ref().and_then(|snapshot| {
        if let MatchStatus::Ended { winner } = snapshot.status {
            Some(winner)
        } else {
            None
        }
    });

    let message = match winner {
        Some(Player::P1) => "Player 1 wins!",
        Some(Player::P2) => "Player 2 wins!",
        // Shouldn't happen (this state is only ever entered because a
        // snapshot just reported Ended), but stays harmless if it does.
        None => "Match ended.",
    };

    commands
        .spawn((
            MatchEndScreenRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.7)),
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new(message),
                TextFont {
                    font_size: bevy::text::FontSize::Px(48.0),
                    ..default()
                },
                TextColor(Color::WHITE),
            ));
        });
}

fn despawn_match_end_screen(
    mut commands: Commands,
    roots: Query<Entity, With<MatchEndScreenRoot>>,
) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}
