//! Composites a Character at runtime from separate torso/arm/leg sprite
//! entities, per ADR 0007, plus the face-photo slot from ADR 0006. See
//! `docs/issues/combat-foundation/character-rig-compositing.md`.
//!
//! Client-only (rendering has no place on the headless server), so this
//! lives alongside `connect_screen` rather than in the shared lib crate.
//!
//! ## Why there's no per-(Body Type, Facing) socket-offset table
//!
//! ADR 0007 anticipates "socket data" bounded by two Facings times three
//! Body Types. In practice the committed art already bakes that in: for a
//! given Body Type, the torso/arm/leg PNGs all share one identical canvas
//! size, with each part's shape pre-positioned within that shared canvas
//! (confirmed by inspecting the actual files - e.g. medium's torso, both
//! arm poses, and both leg poses are all exactly 690x600). Compositing is
//! therefore just stacking same-size sprites at the same origin with a
//! per-part Z for layering - the "two Facings" axis collapses to a single
//! `flip_x` per part instead of a second set of offsets.
//!
//! The one thing that *does* need real offset math is the face-photo slot,
//! since it isn't part of the shared canvas trick (it's a separate entity
//! layered over the torso's head circle) - `head_local_offset` below is
//! that math, and it's covered by unit tests since it's pure arithmetic.
//!
//! `spawn_character` has no caller yet - actually placing Characters (on
//! the Stage, driven by server state) is later issues' job
//! (`idle-character-on-stage.md`, `render-characters-from-server-state.md`).
//! This module is built ahead of its first production consumer, verified
//! for now by its unit tests plus a one-off manual run; suppress the
//! resulting "never used" noise rather than let it mask real dead code
//! once those issues land.
#![allow(dead_code)]

use bevy::prelude::*;
use combat_ductus::combat::Facing;

/// One of the three cosmetic sprite proportions (ADR 0003). Carries no
/// gameplay effect - purely which art assets get loaded.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyType {
    Small,
    Medium,
    Large,
}

/// A `combat::Facing` value attached as a component. `combat` deliberately
/// has no Bevy dependency at all (see its module doc comment), so `Facing`
/// itself can't derive `Component` - this newtype is the rendering side's
/// own wrapper around the same value instead of adding a Bevy dependency to
/// the pure combat-rules module just for this.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CharacterFacing(pub Facing);

/// Which pose the arm/leg entity is currently showing. Punch/Kick input
/// handling (a later issue) drives this by mutating the component; this
/// module only has to react to it correctly, per the "swapping doesn't
/// require moving or re-anchoring the torso" acceptance criterion.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LimbPose {
    #[default]
    Idle,
    Attacking,
}

/// Marker distinguishing the arm entity from the leg entity, since both
/// carry a `LimbPose` and need the same "look up my asset + z" system.
///
/// `pub(crate)` (not private) so other client-only modules - e.g. the code
/// driving Punch/Kick's swing from the server's `attacking` field on a
/// snapshot - can tell which limb entity is which without needing to know
/// anything else about how the rig is put together.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Limb {
    Arm,
    Leg,
}

/// Z-order per ADR 0007: idle limbs sit behind the torso; an attacking limb
/// (arm on Punch, leg on Kick) is drawn in front of it instead. The face
/// slot always sits above everything else on the Character.
const Z_LIMB_IDLE: f32 = 0.0;
const Z_TORSO: f32 = 1.0;
const Z_LIMB_ATTACKING: f32 = 2.0;
const Z_FACE: f32 = 3.0;

/// Horizontal squash factor for the face-photo slot (ADR 0006) - narrows
/// the (viewer-facing) photo to suggest it has turned toward the direction
/// the Character faces. A fixed magnitude is correct for both Facings: the
/// squash itself doesn't need to differ left vs right, only the slot's
/// *position* does (see `head_local_offset`), since flipping a uniform
/// horizontal scale looks identical before and after a mirror.
const FACE_SQUASH_X: f32 = 0.78;
/// The placeholder face fills most of the head circle, leaving a visible
/// ring of the head silhouette's own outline around it.
const FACE_SIZE_RATIO: f32 = 0.82;
const PLACEHOLDER_FACE_COLOR: Color = Color::srgb(0.86, 0.67, 0.53);

struct BodyTypeGeometry {
    asset_prefix: &'static str,
    canvas_size: Vec2,
    head_center: Vec2,
    head_diameter: f32,
    /// Image-space Y of the sole of the idle-pose foot (the lowest opaque
    /// pixel of `leg_idle`) - lets callers (e.g. placing a Character on
    /// the Stage) align feet to a floor line instead of guessing.
    feet_bottom_y: f32,
}

/// Pixel geometry measured directly from the committed art (see the module
/// doc comment) - canvas size, per body type, is shared across every part;
/// head center/diameter come from the light-fill circle in the torso PNG.
/// Image-space (Y grows downward, origin top-left); converted to Bevy's
/// Y-up local space in `head_local_offset`/`feet_local_offset_y`.
fn body_type_geometry(body_type: BodyType) -> BodyTypeGeometry {
    match body_type {
        BodyType::Small => BodyTypeGeometry {
            asset_prefix: "small",
            canvas_size: Vec2::new(598.0, 520.0),
            head_center: Vec2::new(185.5, 77.5),
            head_diameter: 121.0,
            feet_bottom_y: 512.0,
        },
        BodyType::Medium => BodyTypeGeometry {
            asset_prefix: "medium",
            canvas_size: Vec2::new(690.0, 600.0),
            head_center: Vec2::new(214.5, 89.5),
            head_diameter: 141.0,
            feet_bottom_y: 590.0,
        },
        BodyType::Large => BodyTypeGeometry {
            asset_prefix: "large",
            canvas_size: Vec2::new(782.0, 680.0),
            head_center: Vec2::new(243.5, 101.5),
            head_diameter: 159.0,
            feet_bottom_y: 669.0,
        },
    }
}

fn facing_sign(facing: Facing) -> f32 {
    match facing {
        Facing::Right => 1.0,
        Facing::Left => -1.0,
    }
}

/// The face slot's position, local to the Character root (same origin the
/// torso/arm/leg sprites use). Art is authored for `Facing::Right`; the
/// body-part sprites handle mirroring for `Facing::Left` via `flip_x`, but
/// `flip_x` only mirrors a sprite's own texture in place; it doesn't move a
/// separate child entity's Transform. So the face slot's *own* position
/// has to be mirrored by hand here to track where the (also-flipped) torso
/// texture's head circle actually ends up.
pub(crate) fn head_local_offset(body_type: BodyType, facing: Facing) -> Vec2 {
    let geometry = body_type_geometry(body_type);
    let canonical_x = geometry.head_center.x - geometry.canvas_size.x / 2.0;
    let y = geometry.canvas_size.y / 2.0 - geometry.head_center.y;
    Vec2::new(facing_sign(facing) * canonical_x, y)
}

/// How far below the Character root's own origin (the same origin the
/// torso/arm/leg sprites share) the sole of the foot sits, at scale 1.0.
/// Facing-independent - mirroring is purely horizontal, and this is a Y
/// value. Callers placing a Character on a floor line (e.g. the Stage)
/// use this instead of guessing an offset by eye.
pub fn feet_local_offset_y(body_type: BodyType) -> f32 {
    let geometry = body_type_geometry(body_type);
    geometry.canvas_size.y / 2.0 - geometry.feet_bottom_y
}

fn limb_asset_suffix(limb: Limb, pose: LimbPose) -> &'static str {
    match (limb, pose) {
        (Limb::Arm, LimbPose::Idle) => "arm_idle",
        (Limb::Arm, LimbPose::Attacking) => "arm_punch",
        (Limb::Leg, LimbPose::Idle) => "leg_idle",
        (Limb::Leg, LimbPose::Attacking) => "leg_kick",
    }
}

fn limb_z(pose: LimbPose) -> f32 {
    match pose {
        LimbPose::Idle => Z_LIMB_IDLE,
        LimbPose::Attacking => Z_LIMB_ATTACKING,
    }
}

/// Spawns a fully composited Character (torso + arm + leg + placeholder
/// face) at `position`, and returns the root entity. The root carries
/// `BodyType` and `Facing` for later systems (e.g. rendering from server
/// state) to read back.
/// The entities a composited Character is made of. Callers that only care
/// about the Character as a whole (position, despawning) just need `root`;
/// callers driving Punch/Kick's swing (e.g. from a server snapshot's
/// `attacking` field) need `arm`/`leg` too, since `LimbPose` lives on those
/// child entities, not the root.
pub struct CharacterEntities {
    pub root: Entity,
    pub arm: Entity,
    pub leg: Entity,
    pub face: Entity,
}

pub fn spawn_character(
    commands: &mut Commands,
    asset_server: &AssetServer,
    body_type: BodyType,
    facing: Facing,
    initial_arm_pose: LimbPose,
    initial_leg_pose: LimbPose,
    position: Vec2,
) -> CharacterEntities {
    let geometry = body_type_geometry(body_type);
    let flip = matches!(facing, Facing::Left);
    let load = |suffix: &str| -> Handle<Image> {
        asset_server.load(format!("characters/{}_{suffix}.png", geometry.asset_prefix))
    };

    let torso = commands
        .spawn((
            Sprite {
                image: load("torso"),
                flip_x: flip,
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, Z_TORSO),
        ))
        .id();

    let arm = commands
        .spawn((
            Sprite {
                image: load(limb_asset_suffix(Limb::Arm, initial_arm_pose)),
                flip_x: flip,
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, limb_z(initial_arm_pose)),
            Limb::Arm,
            initial_arm_pose,
            body_type,
            CharacterFacing(facing),
        ))
        .id();

    let leg = commands
        .spawn((
            Sprite {
                image: load(limb_asset_suffix(Limb::Leg, initial_leg_pose)),
                flip_x: flip,
                ..default()
            },
            Transform::from_xyz(0.0, 0.0, limb_z(initial_leg_pose)),
            Limb::Leg,
            initial_leg_pose,
            body_type,
            CharacterFacing(facing),
        ))
        .id();

    let head_offset = head_local_offset(body_type, facing);
    let face = commands
        .spawn((
            Sprite::from_color(
                PLACEHOLDER_FACE_COLOR,
                Vec2::splat(geometry.head_diameter * FACE_SIZE_RATIO),
            ),
            Transform::from_xyz(head_offset.x, head_offset.y, Z_FACE).with_scale(Vec3::new(
                FACE_SQUASH_X,
                1.0,
                1.0,
            )),
        ))
        .id();

    let root = commands
        .spawn((
            body_type,
            CharacterFacing(facing),
            Transform::from_xyz(position.x, position.y, 0.0),
            Visibility::default(),
        ))
        .add_children(&[torso, arm, leg, face])
        .id();

    CharacterEntities {
        root,
        arm,
        leg,
        face,
    }
}

/// Reacts to `LimbPose` changing on an arm/leg entity by swapping its
/// sprite image and Z - the torso is never touched, which is exactly what
/// "swapping doesn't require moving or re-anchoring the torso" means.
fn update_limb_sprites(
    asset_server: Res<AssetServer>,
    mut limbs: Query<(&Limb, &LimbPose, &BodyType, &mut Sprite, &mut Transform), Changed<LimbPose>>,
) {
    for (limb, pose, body_type, mut sprite, mut transform) in &mut limbs {
        let geometry = body_type_geometry(*body_type);
        sprite.image = asset_server.load(format!(
            "characters/{}_{}.png",
            geometry.asset_prefix,
            limb_asset_suffix(*limb, *pose)
        ));
        transform.translation.z = limb_z(*pose);
    }
}

pub struct CharacterRigPlugin;

impl Plugin for CharacterRigPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, update_limb_sprites);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_offset_mirrors_between_facings() {
        let right = head_local_offset(BodyType::Medium, Facing::Right);
        let left = head_local_offset(BodyType::Medium, Facing::Left);

        // Same vertical position (flip_x never touches Y)...
        assert_eq!(right.y, left.y);
        // ...but the horizontal position mirrors around the canvas center.
        assert_eq!(right.x, -left.x);
    }

    #[test]
    fn head_offset_matches_measured_art_for_medium_right() {
        // Measured directly from medium_torso.png: canvas 690x600, head
        // circle center (214.5, 89.5). Facing::Right is the canonical,
        // unflipped orientation the art is authored for.
        let offset = head_local_offset(BodyType::Medium, Facing::Right);
        assert!((offset.x - (-130.5)).abs() < f32::EPSILON);
        assert!((offset.y - 210.5).abs() < f32::EPSILON);
    }

    #[test]
    fn feet_offset_matches_measured_art_for_medium() {
        // Measured directly from medium_leg_idle.png: canvas height 600,
        // lowest opaque pixel (the sole) at y=590.
        let offset = feet_local_offset_y(BodyType::Medium);
        assert!((offset - (-290.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn feet_sit_below_the_character_origin_for_every_body_type() {
        for body_type in [BodyType::Small, BodyType::Medium, BodyType::Large] {
            assert!(feet_local_offset_y(body_type) < 0.0);
        }
    }

    #[test]
    fn every_body_type_has_a_distinct_asset_prefix() {
        let prefixes: Vec<_> = [BodyType::Small, BodyType::Medium, BodyType::Large]
            .into_iter()
            .map(|body_type| body_type_geometry(body_type).asset_prefix)
            .collect();
        assert_eq!(prefixes, ["small", "medium", "large"]);
    }

    #[test]
    fn limb_asset_suffix_covers_every_limb_pose_combination() {
        assert_eq!(limb_asset_suffix(Limb::Arm, LimbPose::Idle), "arm_idle");
        assert_eq!(
            limb_asset_suffix(Limb::Arm, LimbPose::Attacking),
            "arm_punch"
        );
        assert_eq!(limb_asset_suffix(Limb::Leg, LimbPose::Idle), "leg_idle");
        assert_eq!(
            limb_asset_suffix(Limb::Leg, LimbPose::Attacking),
            "leg_kick"
        );
    }

    #[test]
    fn attacking_pose_renders_in_front_of_idle_pose() {
        assert!(limb_z(LimbPose::Attacking) > Z_TORSO);
        assert!(limb_z(LimbPose::Idle) < Z_TORSO);
    }
}
