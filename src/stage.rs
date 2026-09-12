//! The single fixed Stage (a cleared office landscape, per `CONTEXT.md`)
//! and its static camera. See
//! `docs/issues/combat-foundation/stage-and-camera.md`.
//!
//! Owns the app's only camera: the Connect screen's UI and any future
//! world-space rendering (Characters, VFX) all share it rather than each
//! spawning their own, since a second camera would need explicit
//! render-target/order configuration to avoid ambiguity. It never moves,
//! follows, or zooms dynamically - v1 has exactly one Stage with no
//! selection, and nothing in the domain model calls for camera movement.

use bevy::camera::{ClearColorConfig, OrthographicProjection, ScalingMode};
use bevy::prelude::*;

/// The Stage background art's native resolution. The camera's `AutoMin`
/// scaling mode treats this as the minimum guaranteed-visible area: at
/// exactly this window aspect ratio the Stage renders pixel-for-pixel, and
/// other aspect ratios scale to keep the whole width or height visible
/// (whichever the window is narrower/shorter than) rather than
/// letterboxing - "full-frame, no visible letterboxing/cropping at typical
/// web viewport sizes" is exactly what `AutoMin` is for.
const STAGE_WIDTH: f32 = 1280.0;
const STAGE_HEIGHT: f32 = 720.0;

/// The Stage art's floor color, used as the camera's clear color: at a
/// window wider than 16:9, `AutoMin` reveals world space beyond the
/// background sprite's left/right edges, and this keeps that reveal a
/// blend rather than a stark border. It only matches the floor band, not
/// the lighter wall band above the floor line - an acceptable seam for
/// placeholder art; widening the background image itself would be the
/// real fix later.
const CLEAR_COLOR: Color = Color::srgb(0.612, 0.639, 0.686);

pub struct StagePlugin;

impl Plugin for StagePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_camera_and_stage);
    }
}

fn spawn_camera_and_stage(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(CLEAR_COLOR),
            ..default()
        },
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: STAGE_WIDTH,
                min_height: STAGE_HEIGHT,
            },
            ..OrthographicProjection::default_2d()
        }),
    ));

    commands.spawn((
        Sprite::from_image(asset_server.load("stages/office.png")),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}
