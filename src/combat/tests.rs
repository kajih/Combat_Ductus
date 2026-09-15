//! Unit tests for `combat` - the one test suite in this crate that is
//! purely unit tests (see the parent module's doc comment): every rule
//! here is exercised directly, with no rendering or networking involved,
//! unlike `client_net`'s, `server_net`'s and `connect_screen`'s, which all
//! run against a real local server.
//!
//! Split by what's under test, not by `combat` API surface: `combat` -
//! Punch/Kick damage, range, animation, cooldown, Match-end and forfeit;
//! `movement` - ground movement and Jump; `special` - Motivational Speech's
//! gating and speech-bubble VFX. All three are still `crate::combat::tests`
//! itself, not separate top-level modules - see `state_at` below, shared
//! by all three via ordinary private-to-ancestor visibility.

use super::*;

mod combat;
mod movement;
mod special;

fn state_at(position: f32, facing: Facing) -> CharacterState {
    CharacterState::new(position, facing)
}
