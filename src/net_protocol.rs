//! Shared wire message types exchanged between the `client` and `server`
//! binaries over a WebSocket connection, JSON-encoded.
//!
//! This module does no I/O of its own — it's plain `serde`-derived data, so
//! it compiles unchanged on both the native target and
//! `wasm32-unknown-unknown`. Actually opening a socket and reading/writing
//! these types is the job of `server_net` (server-side) and, in a later
//! issue, the client's own connection module.

use crate::combat::{Attack, Facing, Player};
use serde::{Deserialize, Serialize};

/// A single input event sent from the client to the server for the tick it
/// occurred on. Movement is edge-triggered (key down/up), matching the
/// controls decision (A/D move, Space jump, J punch, K kick, H special).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InputEvent {
    /// The move-left (A) key was pressed or released.
    MoveLeft(bool),
    /// The move-right (D) key was pressed or released.
    MoveRight(bool),
    /// The jump (Space) key was pressed.
    Jump,
    /// The punch (J) key was pressed.
    Punch,
    /// The kick (K) key was pressed.
    Kick,
    /// The special (H) key was pressed.
    Special,
    /// The restart control on the Match-Ended screen was activated. Not
    /// tied to a specific player the way the other variants are - the
    /// server only honors this once the Match has actually ended (see
    /// `restart-match.md`), regardless of who sent it.
    RequestRestart,
}

impl InputEvent {
    /// The `combat::Attack` this input triggers, if it's an attack input at
    /// all (movement/jump/restart events return `None`).
    pub fn as_attack(self) -> Option<Attack> {
        match self {
            InputEvent::Punch => Some(Attack::Punch),
            InputEvent::Kick => Some(Attack::Kick),
            InputEvent::Special => Some(Attack::Special),
            InputEvent::MoveLeft(_)
            | InputEvent::MoveRight(_)
            | InputEvent::Jump
            | InputEvent::RequestRestart => None,
        }
    }
}

/// A snapshot of one Character's rendered-relevant state, sent every tick.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CharacterSnapshot {
    pub position: f32,
    pub facing: Facing,
    pub health: u8,
    pub airborne: bool,
    /// How high off the ground this Character currently is, in the same
    /// abstract game-position units as `position`. Zero whenever `airborne`
    /// is false. Rendering-only.
    pub vertical_offset: f32,
    /// Whether this Character's Motivational Speech speech-bubble VFX is
    /// currently showing - only ever true after a *successful* Special
    /// cast (a rejected/gated attempt has no feedback at all).
    pub speaking: bool,
    /// Which attack's arm/leg swing is currently animating, if any - only
    /// ever `Punch` or `Kick` (Special's feedback is a separate VFX, not a
    /// limb swing). Set the instant the attack is thrown, regardless of
    /// whether it lands, and cleared automatically a short time later.
    pub attacking: Option<Attack>,
}

/// Whether the Match is still being played or has already ended, and if so
/// by whom. Terminal once `Ended` — v1 has no rematch/restart flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStatus {
    InProgress,
    Ended { winner: Player },
}

/// The full state snapshot broadcast from server to client every tick.
/// Carries everything the client needs to render a frame - it never
/// simulates anything itself (ADR 0005).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub tick: u64,
    pub p1: CharacterSnapshot,
    pub p2: CharacterSnapshot,
    pub status: MatchStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T>(value: &T) -> T
    where
        T: Serialize + for<'de> Deserialize<'de>,
    {
        let json = serde_json::to_string(value).expect("serialize");
        serde_json::from_str(&json).expect("deserialize")
    }

    #[test]
    fn input_events_round_trip() {
        for event in [
            InputEvent::MoveLeft(true),
            InputEvent::MoveLeft(false),
            InputEvent::MoveRight(true),
            InputEvent::Jump,
            InputEvent::Punch,
            InputEvent::Kick,
            InputEvent::Special,
            InputEvent::RequestRestart,
        ] {
            assert_eq!(round_trip(&event), event);
        }
    }

    #[test]
    fn state_snapshot_round_trips() {
        let snapshot = StateSnapshot {
            tick: 42,
            p1: CharacterSnapshot {
                position: -1.5,
                facing: Facing::Right,
                health: 3,
                airborne: false,
                vertical_offset: 0.0,
                speaking: true,
                attacking: Some(Attack::Punch),
            },
            p2: CharacterSnapshot {
                position: 1.5,
                facing: Facing::Left,
                health: 5,
                airborne: true,
                vertical_offset: 0.6,
                speaking: false,
                attacking: None,
            },
            status: MatchStatus::InProgress,
        };

        assert_eq!(round_trip(&snapshot), snapshot);
    }

    #[test]
    fn ended_match_status_round_trips_with_winner() {
        let status = MatchStatus::Ended { winner: Player::P2 };
        assert_eq!(round_trip(&status), status);
    }
}
