//! Spawns Player 1 and Player 2's Characters on entering the Match, and
//! keeps them positioned from the server's state snapshots instead of any
//! local placeholder. See
//! `docs/issues/combat-foundation/render-characters-from-server-state.md`.
//!
//! Replaces the M0-era `idle_character` module's hardcoded single-Character
//! spawn (per that issue's own note that this would happen). Player 2
//! renders as a stationary Idle Opponent simply because nothing in this
//! milestone moves it server-side yet - there's no special-casing here for
//! that, it falls out of "position always reflects the server's state".

use crate::character_rig::{self, BodyType, LimbPose};
use crate::connect_screen::{AppState, LatestSnapshot};
use bevy::prelude::*;
use combat_ductus::combat::Facing;
use combat_ductus::net_protocol::CharacterSnapshot;

/// Scales the rig's native pixel-sized art down to something proportionate
/// against the Stage's office backdrop. Same value M0's idle_character
/// module used, picked by eye against a screenshot.
const CHARACTER_SCALE: f32 = 0.6;

/// World Y the Characters' feet rest on - the Stage art's floor line. Same
/// measurement M0's idle_character module used (see that history for how
/// it was derived).
const FLOOR_Y: f32 = -86.0;

/// Converts `combat`'s abstract game-position units (e.g. `ATTACK_RANGE`,
/// which is small - a couple of units) into Stage world/pixel space. Picked
/// so the two Characters' starting spread reads as a plausible distance
/// apart on the Stage, not so the number itself means anything physical.
const POSITION_SCALE: f32 = 100.0;

/// Which Match participant a Character entity represents - purely a
/// rendering-side tag for picking the right half of the snapshot to read;
/// unrelated to (and not a duplicate of) `combat::Player`, which is
/// server-side simulation state this module never touches directly.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    P1,
    P2,
}

pub struct MatchCharactersPlugin;

impl Plugin for MatchCharactersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), spawn_match_characters)
            .add_systems(OnExit(AppState::InMatch), despawn_match_characters)
            .add_systems(
                Update,
                update_character_positions.run_if(in_state(AppState::InMatch)),
            );
    }
}

/// World Y for a Character's root so its feet land on `FLOOR_Y`, at
/// `CHARACTER_SCALE`.
fn root_y() -> f32 {
    FLOOR_Y - character_rig::feet_local_offset_y(BodyType::Medium) * CHARACTER_SCALE
}

/// `combat`'s starting positions (see `combat::MatchState::new`) converted
/// to Stage world space - used only as the very first frame's placement,
/// before any real snapshot has arrived to correct it.
fn default_world_x(slot: Slot) -> f32 {
    let game_position = match slot {
        Slot::P1 => -3.0,
        Slot::P2 => 3.0,
    };
    game_position * POSITION_SCALE
}

fn spawn_match_characters(mut commands: Commands, asset_server: Res<AssetServer>) {
    for (slot, facing) in [(Slot::P1, Facing::Right), (Slot::P2, Facing::Left)] {
        let x = default_world_x(slot);
        let y = root_y();

        let entity = character_rig::spawn_character(
            &mut commands,
            &asset_server,
            BodyType::Medium,
            facing,
            LimbPose::Idle,
            LimbPose::Idle,
            Vec2::new(x, y),
        );

        commands.entity(entity).insert((
            slot,
            Transform::from_xyz(x, y, 0.0).with_scale(Vec3::splat(CHARACTER_SCALE)),
        ));
    }
}

fn despawn_match_characters(mut commands: Commands, characters: Query<Entity, With<Slot>>) {
    for entity in &characters {
        commands.entity(entity).despawn();
    }
}

fn update_character_positions(
    latest_snapshot: Res<LatestSnapshot>,
    mut characters: Query<(&Slot, &mut Transform)>,
) {
    let Some(snapshot) = &latest_snapshot.0 else {
        return;
    };

    for (slot, mut transform) in &mut characters {
        let character_snapshot: &CharacterSnapshot = match slot {
            Slot::P1 => &snapshot.p1,
            Slot::P2 => &snapshot.p2,
        };
        transform.translation.x = character_snapshot.position * POSITION_SCALE;
        transform.translation.y = root_y();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_positions_place_p1_left_of_p2() {
        assert!(default_world_x(Slot::P1) < 0.0);
        assert!(default_world_x(Slot::P2) > 0.0);
    }

    #[test]
    fn default_positions_are_symmetric() {
        assert_eq!(default_world_x(Slot::P1), -default_world_x(Slot::P2));
    }
}
