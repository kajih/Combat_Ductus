//! Places one idle Character on the Stage - completes M0 (a Character
//! rendered correctly, with no input, combat, or networking involved). See
//! `docs/issues/combat-foundation/idle-character-on-stage.md`.
//!
//! This hardcoded single-Character spawn is temporary scaffolding:
//! `render-characters-from-server-state.md` (M1) replaces it with two
//! Characters - Player 1 and the Idle Opponent - positioned from the
//! server's actual state snapshots instead of a fixed spot picked here.

use crate::character_rig::{self, BodyType, LimbPose};
use bevy::prelude::*;
use combat_ductus::combat::Facing;

/// Scales the rig's native pixel-sized art down to something proportionate
/// against the Stage's office backdrop (its desks/monitors read as
/// human-scale, not sized for a ~600px-tall figure). Picked by eye against
/// a screenshot, not derived from any specific measurement.
const CHARACTER_SCALE: f32 = 0.6;

/// World Y the Character's feet rest on. Measured directly from the Stage
/// art: the wall/floor divider stroke sits at image-space y ~= 444-449 out
/// of the Stage's 720px height; using its midpoint (446) and converting to
/// Bevy's Y-up local space around the Stage sprite's center anchor:
/// 720/2 - 446 = -86.
const FLOOR_Y: f32 = -86.0;

pub struct IdleCharacterPlugin;

impl Plugin for IdleCharacterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_idle_character);
    }
}

fn spawn_idle_character(mut commands: Commands, asset_server: Res<AssetServer>) {
    let body_type = BodyType::Medium;
    let feet_local_y = character_rig::feet_local_offset_y(body_type) * CHARACTER_SCALE;
    let root_y = FLOOR_Y - feet_local_y;

    let entity = character_rig::spawn_character(
        &mut commands,
        &asset_server,
        body_type,
        Facing::Right,
        LimbPose::Idle,
        LimbPose::Idle,
        Vec2::new(0.0, root_y),
    );

    // spawn_character already sets the root Transform to `position`; this
    // re-insert only adds the uniform scale (its API has no scale
    // parameter, since scale is a rendering/placement concern, not part of
    // compositing the rig itself).
    commands
        .entity(entity)
        .insert(Transform::from_xyz(0.0, root_y, 0.0).with_scale(Vec3::splat(CHARACTER_SCALE)));
}
