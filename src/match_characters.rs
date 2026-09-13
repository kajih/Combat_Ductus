//! Spawns Player 1 and Player 2's Characters on entering the Match, and
//! keeps them positioned - and their Punch/Kick swing animating - from the
//! server's state snapshots instead of any local placeholder. See
//! `docs/issues/combat-foundation/render-characters-from-server-state.md`
//! and `docs/issues/combat-foundation/punch-kick-health-depletion.md`.
//!
//! Replaces the M0-era `idle_character` module's hardcoded single-Character
//! spawn (per that issue's own note that this would happen). Player 2
//! renders as a stationary Idle Opponent simply because nothing in this
//! milestone moves it server-side yet - there's no special-casing here for
//! that, it falls out of "position always reflects the server's state".

use crate::character_rig::{self, BodyType, Limb, LimbPose};
use crate::connect_screen::{AppState, LatestSnapshot};
use bevy::prelude::*;
use combat_ductus::combat::{Attack, Facing};
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
/// Present on a Character's root *and* its arm/leg children, since the
/// attack-animation system needs to know whose snapshot to read from a
/// limb entity, not just the root's.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum Slot {
    P1,
    P2,
}

/// Marks specifically the Character *root* entity, as opposed to its
/// torso/arm/leg/face children (which also carry `Slot`) - so despawn and
/// position-update queries don't also try to act on the children directly.
#[derive(Component)]
struct CharacterRoot;

pub struct MatchCharactersPlugin;

impl Plugin for MatchCharactersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InMatch), spawn_match_characters)
            .add_systems(OnExit(AppState::InMatch), despawn_match_characters)
            .add_systems(
                Update,
                (update_character_positions, update_attack_animations)
                    .run_if(in_state(AppState::InMatch)),
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

        let entities = character_rig::spawn_character(
            &mut commands,
            &asset_server,
            BodyType::Medium,
            facing,
            LimbPose::Idle,
            LimbPose::Idle,
            Vec2::new(x, y),
        );

        commands.entity(entities.root).insert((
            slot,
            CharacterRoot,
            Transform::from_xyz(x, y, 0.0).with_scale(Vec3::splat(CHARACTER_SCALE)),
        ));
        commands.entity(entities.arm).insert(slot);
        commands.entity(entities.leg).insert(slot);
    }
}

fn despawn_match_characters(mut commands: Commands, roots: Query<Entity, With<CharacterRoot>>) {
    for entity in &roots {
        commands.entity(entity).despawn();
    }
}

fn character_snapshot_for(
    snapshot: &combat_ductus::net_protocol::StateSnapshot,
    slot: Slot,
) -> &CharacterSnapshot {
    match slot {
        Slot::P1 => &snapshot.p1,
        Slot::P2 => &snapshot.p2,
    }
}

fn update_character_positions(
    latest_snapshot: Res<LatestSnapshot>,
    mut characters: Query<(&Slot, &mut Transform), With<CharacterRoot>>,
) {
    let Some(snapshot) = &latest_snapshot.0 else {
        return;
    };

    for (slot, mut transform) in &mut characters {
        let character_snapshot = character_snapshot_for(snapshot, *slot);
        transform.translation.x = character_snapshot.position * POSITION_SCALE;
        transform.translation.y = root_y() + character_snapshot.vertical_offset * POSITION_SCALE;
    }
}

/// Drives each limb's `LimbPose` from the corresponding Character's
/// `attacking` field - the arm swings on Punch, the leg swings on Kick,
/// regardless of whether the attack actually landed (a player should see
/// their attack attempt even on a whiff). The rest (swapping the sprite
/// image/Z for the new pose) is `character_rig::update_limb_sprites`'s job,
/// reacting to `LimbPose` changing - this system only decides what the
/// pose *should* be.
fn update_attack_animations(
    latest_snapshot: Res<LatestSnapshot>,
    mut limbs: Query<(&Slot, &Limb, &mut LimbPose)>,
) {
    let Some(snapshot) = &latest_snapshot.0 else {
        return;
    };

    for (slot, limb, mut pose) in &mut limbs {
        let character_snapshot = character_snapshot_for(snapshot, *slot);
        let desired = match (character_snapshot.attacking, limb) {
            (Some(Attack::Punch), Limb::Arm) => LimbPose::Attacking,
            (Some(Attack::Kick), Limb::Leg) => LimbPose::Attacking,
            _ => LimbPose::Idle,
        };
        // Compare before writing - an unconditional assignment would mark
        // LimbPose "changed" every frame regardless of whether the value
        // actually differs, needlessly re-triggering the sprite-swap
        // system downstream (the same kind of per-frame churn the Health
        // HUD had before it learned to check first).
        if *pose != desired {
            *pose = desired;
        }
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
