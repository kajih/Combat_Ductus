//! Captures gameplay keyboard input (A/D movement, for now) and forwards
//! it to the server as `net_protocol::InputEvent`s - the client itself
//! never simulates movement, per ADR 0005; it only reports which keys are
//! down and renders whatever position the server reports back. See
//! `docs/issues/combat-foundation/ground-movement.md`.

use crate::connect_screen::{AppState, ConnectionSlot};
use bevy::prelude::*;
use combat_ductus::net_protocol::InputEvent;

pub struct PlayerInputPlugin;

impl Plugin for PlayerInputPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            send_movement_input.run_if(in_state(AppState::InMatch)),
        );
    }
}

fn send_movement_input(keys: Res<ButtonInput<KeyCode>>, mut slot: NonSendMut<ConnectionSlot>) {
    // Edge-triggered (fires once on key-down/up, not every frame a key is
    // held) - matches the controls decision (A/D move, Space jump, J
    // punch, K kick, H special) and keeps the server from seeing a flood
    // of redundant "still pressed" events every tick.
    if keys.just_pressed(KeyCode::KeyA) {
        slot.send_input(InputEvent::MoveLeft(true));
    }
    if keys.just_released(KeyCode::KeyA) {
        slot.send_input(InputEvent::MoveLeft(false));
    }
    if keys.just_pressed(KeyCode::KeyD) {
        slot.send_input(InputEvent::MoveRight(true));
    }
    if keys.just_released(KeyCode::KeyD) {
        slot.send_input(InputEvent::MoveRight(false));
    }
}
