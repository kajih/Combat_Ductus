//! The headless Bevy simulation: steps `combat::MatchState` on a fixed
//! ~30Hz schedule regardless of whether a client is connected, applying
//! whatever input/disconnect signals have arrived from `super::accept`
//! over the channels `super` defines, and broadcasting a snapshot every
//! tick. See the parent module's doc comment for the full picture.

use super::{Disconnection, TaggedInputEvent};
use crate::combat::{MOVE_SPEED_PER_TICK, MatchEndReason, MatchState, Player};
use crate::net_protocol::{
    CharacterSnapshot, InputEvent, MatchStatus, ServerMessage, StateSnapshot,
};
use bevy::app::ScheduleRunnerPlugin;
use bevy::prelude::*;
use crossbeam_channel::Receiver as CbReceiver;
use std::time::Duration;
use tokio::sync::broadcast;

/// The simulation steps ~30 times per second.
const TICK_RATE_HZ: f64 = 30.0;

#[derive(Resource)]
struct MatchStateRes(MatchState);

#[derive(Resource)]
struct IncomingEvents(CbReceiver<TaggedInputEvent>);

/// Which movement keys are currently held, per player slot. Both Players
/// get their own pair now that a second real client can connect and
/// control Player 2 (see `second-client-controls-player-two.md`) - Player
/// 2's fields simply stay `false` for as long as it's an unclaimed Idle
/// Opponent, since nothing ever sends it movement input.
#[derive(Resource, Default)]
struct HeldMovement {
    p1_left: bool,
    p1_right: bool,
    p2_left: bool,
    p2_right: bool,
}

impl HeldMovement {
    fn set_left(&mut self, player: Player, pressed: bool) {
        match player {
            Player::P1 => self.p1_left = pressed,
            Player::P2 => self.p2_left = pressed,
        }
    }

    fn set_right(&mut self, player: Player, pressed: bool) {
        match player {
            Player::P1 => self.p1_right = pressed,
            Player::P2 => self.p2_right = pressed,
        }
    }

    /// Releases both of `player`'s held movement keys - used when their
    /// connection disconnects, so they stop walking on their own instead
    /// of continuing in whatever direction was last held.
    fn reset(&mut self, player: Player) {
        self.set_left(player, false);
        self.set_right(player, false);
    }
}

#[derive(Resource)]
struct OutgoingSnapshots(broadcast::Sender<String>);

#[derive(Resource)]
struct DisconnectedConnections(CbReceiver<Disconnection>);

/// Run the headless simulation loop. Blocks forever (this is the server
/// binary's whole reason to exist), stepping `combat::MatchState` on a
/// fixed ~30Hz schedule regardless of whether a client is connected -
/// Player 2 exists as a stationary Idle Opponent from the very first tick.
pub fn run_bevy_app(
    incoming_rx: CbReceiver<TaggedInputEvent>,
    outgoing_tx: broadcast::Sender<String>,
    disconnected_rx: CbReceiver<Disconnection>,
) {
    App::new()
        .add_plugins(
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / TICK_RATE_HZ,
            ))),
        )
        .insert_resource(MatchStateRes(MatchState::new()))
        .insert_resource(IncomingEvents(incoming_rx))
        .insert_resource(HeldMovement::default())
        .insert_resource(OutgoingSnapshots(outgoing_tx))
        .insert_resource(DisconnectedConnections(disconnected_rx))
        .add_systems(Update, step_simulation)
        .run();
}

fn step_simulation(
    mut match_state: ResMut<MatchStateRes>,
    incoming: Res<IncomingEvents>,
    mut held: ResMut<HeldMovement>,
    outgoing: Res<OutgoingSnapshots>,
    disconnected: Res<DisconnectedConnections>,
) {
    // A connection ending - reset whatever movement it left held, or that
    // Character keeps walking in that direction forever with no one left
    // to release the key. `None` (a spectator disconnecting) never held
    // any movement in the first place, so there's nothing to reset, and
    // never forfeits the Match either. `try_recv` draining a channel that
    // may have more than one queued signal is harmless - the reset is
    // idempotent, and `MatchState::forfeit` already no-ops once the Match
    // has ended (including from an earlier forfeit in the same drain).
    while let Ok(Disconnection {
        slot,
        opponent_connected,
    }) = disconnected.0.try_recv()
    {
        if let Some(player) = slot {
            held.reset(player);
            if opponent_connected {
                match_state.0.forfeit(player);
            }
        }
    }

    while let Ok((slot, event)) = incoming.0.try_recv() {
        match event {
            // A spectator (no assigned slot) has no Character to move -
            // simply drop movement/attack input from one rather than
            // guessing which Character it meant.
            InputEvent::MoveLeft(pressed) => {
                if let Some(player) = slot {
                    held.set_left(player, pressed);
                }
            }
            InputEvent::MoveRight(pressed) => {
                if let Some(player) = slot {
                    held.set_right(player, pressed);
                }
            }
            InputEvent::Jump => {
                if let Some(player) = slot {
                    match_state.0.jump(player);
                }
            }
            InputEvent::RequestRestart => {
                // Restricted to a connection holding a real player slot -
                // a spectator's request is silently ignored, the same way
                // its movement/attack input already is (see
                // `restrict-restart-to-players.md`). Also only honored once
                // the Match has actually ended - this resets a concluded
                // Match, not an active one.
                if slot.is_some() && match_state.0.has_ended() {
                    match_state.0 = MatchState::new();
                    // A player holding a movement key at the instant the
                    // Match ended stops sending input entirely once the
                    // client leaves AppState::InMatch - so a release event
                    // may never arrive, leaving this stale in HeldMovement
                    // forever. Reset both players unconditionally (not just
                    // whichever one might be affected), matching
                    // MatchState::new()'s own "everyone gets a clean slate"
                    // reset. See reset-input-on-match-restart.md.
                    held.reset(Player::P1);
                    held.reset(Player::P2);
                }
            }
            other => {
                if let (Some(player), Some(attack)) = (slot, other.as_attack()) {
                    match_state.0.apply_attack(player, attack);
                }
            }
        }
    }

    // move_player is itself a no-op once the Match has ended, and already
    // clamps to the Stage bounds - no need to guard has_ended() here too.
    if held.p1_left {
        match_state.0.move_player(Player::P1, -MOVE_SPEED_PER_TICK);
    }
    if held.p1_right {
        match_state.0.move_player(Player::P1, MOVE_SPEED_PER_TICK);
    }
    if held.p2_left {
        match_state.0.move_player(Player::P2, -MOVE_SPEED_PER_TICK);
    }
    if held.p2_right {
        match_state.0.move_player(Player::P2, MOVE_SPEED_PER_TICK);
    }

    match_state.0.advance_tick();

    let snapshot = snapshot_from_state(&match_state.0);
    let message = ServerMessage::Snapshot(snapshot);
    if let Ok(json) = serde_json::to_string(&message) {
        // No connected clients yet is a normal state (Idle Opponent era) -
        // an error here just means nobody is subscribed, not a real failure.
        let _ = outgoing.0.send(json);
    }
}

fn snapshot_from_state(state: &MatchState) -> StateSnapshot {
    StateSnapshot {
        tick: state.tick,
        p1: character_snapshot(&state.p1),
        p2: character_snapshot(&state.p2),
        status: match (state.winner, state.end_reason) {
            (Some(winner), Some(reason)) => MatchStatus::Ended { winner, reason },
            (Some(winner), None) => {
                // Shouldn't happen (`MatchState` always sets both together)
                // - falls back to the more common case rather than
                // panicking on a snapshot conversion.
                MatchStatus::Ended {
                    winner,
                    reason: MatchEndReason::Defeated,
                }
            }
            (None, _) => MatchStatus::InProgress,
        },
    }
}

fn character_snapshot(character: &crate::combat::CharacterState) -> CharacterSnapshot {
    CharacterSnapshot {
        position: character.position,
        facing: character.facing,
        health: character.health,
        airborne: character.airborne,
        vertical_offset: character.vertical_offset,
        speaking: character.speaking,
        attacking: character.attack_animation,
    }
}
