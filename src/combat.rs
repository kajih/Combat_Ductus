//! Pure combat rules: Health, damage, range checks, Special gating, and
//! Match-end detection.
//!
//! Deliberately has no dependency on Bevy's `App`/ECS, or on any networking
//! type. It operates on plain state (positions, Facing, tick counters) and
//! returns plain results, so every rule here is exercised directly by the
//! unit tests below with no rendering or networking involved. The
//! server-authoritative simulation (see `server_net`) is the only consumer.

use serde::{Deserialize, Serialize};

/// A Character's facing direction, fixed for the whole Match (ADR 0002).
/// Player 1 always faces right, Player 2 always faces left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Facing {
    Right,
    Left,
}

impl Facing {
    /// +1.0 for Right, -1.0 for Left — the direction along the ground axis
    /// this Facing points toward.
    fn sign(self) -> f32 {
        match self {
            Facing::Right => 1.0,
            Facing::Left => -1.0,
        }
    }
}

/// Which of the two Match participants an action applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Player {
    P1,
    P2,
}

impl Player {
    fn opponent(self) -> Player {
        match self {
            Player::P1 => Player::P2,
            Player::P2 => Player::P1,
        }
    }
}

/// A Character's third attack category alongside Punch and Kick (ADR 0008).
/// Only Motivational Speech exists today, but this stays an enum since more
/// Specials are anticipated by the domain model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Attack {
    Punch,
    Kick,
    Special,
}

pub const STARTING_HEALTH: u8 = 5;

pub const PUNCH_DAMAGE: u8 = 1;
pub const KICK_DAMAGE: u8 = 2;
/// Punch and Kick share the same range — they differ only by damage, per
/// the domain model (`CONTEXT.md`). Real hitbox rectangles are out of scope
/// for M0/M1; this is a simple 1D distance check.
pub const ATTACK_RANGE: f32 = 1.5;

pub const SPECIAL_DAMAGE: u8 = 1;
/// Motivational Speech can only be cast when the opponent is farther away
/// than this floor (ADR 0008) — the opposite of a normal attack's range.
pub const SPECIAL_MIN_RANGE: f32 = 3.0;
/// Ticks between successive Special casts. Placeholder tuning value.
pub const SPECIAL_COOLDOWN_TICKS: u64 = 90;
/// Ticks a Character is locked out of Special after last being hit.
/// Placeholder tuning value.
pub const SPECIAL_LOCKOUT_TICKS: u64 = 60;

/// Ground movement speed, in world units per simulation tick. Placeholder
/// tuning value.
pub const MOVE_SPEED_PER_TICK: f32 = 0.05;
/// Half the Stage's walkable width, in world units, measured from center.
/// A Character's position is clamped to `[-STAGE_HALF_WIDTH,
/// STAGE_HALF_WIDTH]` so it can't walk off the Stage. Placeholder tuning
/// value - exact bounds (and how they relate to the Stage art's actual
/// pixel width) are deferred.
pub const STAGE_HALF_WIDTH: f32 = 6.0;

/// How long a Punch/Kick's visible arm/leg swing lasts, in ticks -
/// triggered the instant the attack is thrown, regardless of whether it
/// lands (a player should see their attack attempt even on a whiff).
/// Placeholder tuning value; ~1/3 second at the server's ~30Hz tick rate.
pub const ATTACK_ANIMATION_TICKS: u8 = 10;

/// One Character's simulated state within a Match.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CharacterState {
    pub position: f32,
    pub facing: Facing,
    pub health: u8,
    pub airborne: bool,
    /// The tick Special was last successfully cast, if ever. Cooldown is
    /// measured from this.
    pub special_last_cast_tick: Option<u64>,
    /// The tick this Character was last hit, if ever. The Special
    /// recent-damage lockout is measured from this.
    pub last_hit_tick: Option<u64>,
    /// Which attack's arm/leg swing is currently animating, if any. Only
    /// ever `Punch` or `Kick` - Special has no limb swing (its feedback is
    /// the speech-bubble VFX, a separate issue).
    pub attack_animation: Option<Attack>,
    attack_animation_ticks_remaining: u8,
}

impl CharacterState {
    pub fn new(position: f32, facing: Facing) -> Self {
        CharacterState {
            position,
            facing,
            health: STARTING_HEALTH,
            airborne: false,
            special_last_cast_tick: None,
            last_hit_tick: None,
            attack_animation: None,
            attack_animation_ticks_remaining: 0,
        }
    }

    fn start_attack_animation(&mut self, attack: Attack) {
        self.attack_animation = Some(attack);
        self.attack_animation_ticks_remaining = ATTACK_ANIMATION_TICKS;
    }

    fn tick_attack_animation(&mut self) {
        if self.attack_animation_ticks_remaining > 0 {
            self.attack_animation_ticks_remaining -= 1;
            if self.attack_animation_ticks_remaining == 0 {
                self.attack_animation = None;
            }
        }
    }
}

/// The full simulated state of one Match, and the single source of truth
/// the server steps every tick (ADR 0005 — the client never simulates).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchState {
    pub tick: u64,
    pub p1: CharacterState,
    pub p2: CharacterState,
    /// `None` while the Match is in progress; set the instant a Character's
    /// Health reaches 0. Terminal — a Match is never restarted in v1.
    pub winner: Option<Player>,
}

impl MatchState {
    pub fn new() -> Self {
        MatchState {
            tick: 0,
            p1: CharacterState::new(-ATTACK_RANGE * 2.0, Facing::Right),
            p2: CharacterState::new(ATTACK_RANGE * 2.0, Facing::Left),
            winner: None,
        }
    }

    fn character(&self, player: Player) -> &CharacterState {
        match player {
            Player::P1 => &self.p1,
            Player::P2 => &self.p2,
        }
    }

    fn character_mut(&mut self, player: Player) -> &mut CharacterState {
        match player {
            Player::P1 => &mut self.p1,
            Player::P2 => &mut self.p2,
        }
    }

    /// Whether the Match has already ended. Once true, no further attacks
    /// or movement should be applied (see the match-end-screen issue).
    pub fn has_ended(&self) -> bool {
        self.winner.is_some()
    }

    /// Advance the tick counter by one. Called once per server simulation
    /// step, regardless of whether any input arrived.
    pub fn advance_tick(&mut self) {
        self.tick += 1;
        self.p1.tick_attack_animation();
        self.p2.tick_attack_animation();
    }

    /// Move `player` by `delta` world units (positive = toward the
    /// positive-X direction, regardless of that Character's own Facing -
    /// Facing never changes, per ADR 0002, and is unrelated to which way
    /// movement pushes a Character), clamped so they can't walk off the
    /// Stage. A no-op once the Match has ended.
    pub fn move_player(&mut self, player: Player, delta: f32) {
        if self.has_ended() {
            return;
        }

        let character = self.character_mut(player);
        character.position =
            (character.position + delta).clamp(-STAGE_HALF_WIDTH, STAGE_HALF_WIDTH);
    }

    /// Attempt `attack` by `attacker` against their opponent at the current
    /// tick. Returns `true` if the attack landed and dealt damage, `false`
    /// if it was rejected (out of range, gated, or the Match already ended).
    ///
    /// On a successful hit this applies damage, records the recent-damage
    /// lockout on the defender, and sets `winner` the instant Health
    /// reaches 0.
    pub fn apply_attack(&mut self, attacker: Player, attack: Attack) -> bool {
        if self.has_ended() {
            return false;
        }

        let tick = self.tick;
        let defender = attacker.opponent();

        let landed = match attack {
            Attack::Punch | Attack::Kick => {
                let a = self.character(attacker);
                let d = self.character(defender);
                in_attack_range(a, d)
            }
            Attack::Special => {
                let a = self.character(attacker);
                let d = self.character(defender);
                can_cast_special(a, d, tick)
            }
        };

        // Punch/Kick's visible swing plays regardless of whether the
        // attack lands - a player should see their attack attempt even on
        // a whiff. Special has no limb swing (separate VFX, separate
        // issue).
        if matches!(attack, Attack::Punch | Attack::Kick) {
            self.character_mut(attacker).start_attack_animation(attack);
        }

        if !landed {
            return false;
        }

        let damage = match attack {
            Attack::Punch => PUNCH_DAMAGE,
            Attack::Kick => KICK_DAMAGE,
            Attack::Special => SPECIAL_DAMAGE,
        };

        if attack == Attack::Special {
            self.character_mut(attacker).special_last_cast_tick = Some(tick);
        }

        let defender_state = self.character_mut(defender);
        defender_state.health = defender_state.health.saturating_sub(damage);
        defender_state.last_hit_tick = Some(tick);

        if defender_state.health == 0 {
            self.winner = Some(attacker);
        }

        true
    }
}

impl Default for MatchState {
    fn default() -> Self {
        Self::new()
    }
}

/// A Punch/Kick lands when the defender is within `ATTACK_RANGE` of the
/// attacker, on the side the attacker is Facing, and not airborne (Jump is
/// purely movement — a Character cannot be hit by, or throw, a ground
/// attack while airborne, per the existing Jump definition).
fn in_attack_range(attacker: &CharacterState, defender: &CharacterState) -> bool {
    if attacker.airborne || defender.airborne {
        return false;
    }

    let offset = defender.position - attacker.position;
    let distance = offset.abs();
    let facing_matches = offset * attacker.facing.sign() >= 0.0;

    distance <= ATTACK_RANGE && facing_matches
}

/// Motivational Speech ignores range/Facing once cast (ADR 0008), so it
/// isn't balanced by proximity — it's balanced by whether the attacker is
/// even allowed to cast it right now:
/// - the defender must be farther than `SPECIAL_MIN_RANGE` away
/// - the attacker's cooldown must have elapsed
/// - the attacker must not be in their own recent-damage lockout window
fn can_cast_special(attacker: &CharacterState, defender: &CharacterState, tick: u64) -> bool {
    let distance = (defender.position - attacker.position).abs();
    if distance <= SPECIAL_MIN_RANGE {
        return false;
    }

    if let Some(last_cast) = attacker.special_last_cast_tick
        && tick.saturating_sub(last_cast) < SPECIAL_COOLDOWN_TICKS
    {
        return false;
    }

    if let Some(last_hit) = attacker.last_hit_tick
        && tick.saturating_sub(last_hit) < SPECIAL_LOCKOUT_TICKS
    {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_at(position: f32, facing: Facing) -> CharacterState {
        CharacterState::new(position, facing)
    }

    #[test]
    fn punch_deals_one_damage_in_range() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Punch));
        assert_eq!(m.p2.health, STARTING_HEALTH - PUNCH_DAMAGE);
    }

    #[test]
    fn kick_deals_two_damage_in_range() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Kick));
        assert_eq!(m.p2.health, STARTING_HEALTH - KICK_DAMAGE);
    }

    #[test]
    fn health_does_not_go_below_zero() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);
        m.p2.health = 1;

        assert!(m.apply_attack(Player::P1, Attack::Kick));
        assert_eq!(m.p2.health, 0);

        // Match already ended - a further attack should be rejected outright.
        m.p2.health = 0;
        assert!(!m.apply_attack(Player::P1, Attack::Punch));
        assert_eq!(m.p2.health, 0);
    }

    #[test]
    fn punch_at_exactly_max_range_lands() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(ATTACK_RANGE, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Punch));
    }

    #[test]
    fn punch_just_beyond_max_range_misses() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(ATTACK_RANGE + 0.01, Facing::Left);

        assert!(!m.apply_attack(Player::P1, Attack::Punch));
        assert_eq!(m.p2.health, STARTING_HEALTH);
    }

    #[test]
    fn punch_misses_when_opponent_is_behind_facing() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(-1.0, Facing::Left);

        assert!(!m.apply_attack(Player::P1, Attack::Punch));
    }

    #[test]
    fn punch_misses_airborne_opponent() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);
        m.p2.airborne = true;

        assert!(!m.apply_attack(Player::P1, Attack::Punch));
    }

    #[test]
    fn airborne_attacker_cannot_punch_or_kick() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p1.airborne = true;
        m.p2 = state_at(1.0, Facing::Left);

        assert!(!m.apply_attack(Player::P1, Attack::Punch));
    }

    #[test]
    fn special_hits_regardless_of_distance_and_facing_when_allowed() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        // Opponent is behind the attacker's Facing and far away - a Punch/Kick
        // would never land here, but Special ignores both.
        m.p2 = state_at(-(SPECIAL_MIN_RANGE + 1.0), Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Special));
        assert_eq!(m.p2.health, STARTING_HEALTH - SPECIAL_DAMAGE);
    }

    #[test]
    fn special_fails_exactly_at_min_range_boundary() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(SPECIAL_MIN_RANGE, Facing::Left);

        assert!(!m.apply_attack(Player::P1, Attack::Special));
    }

    #[test]
    fn special_succeeds_just_beyond_min_range_boundary() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(SPECIAL_MIN_RANGE + 0.01, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Special));
    }

    #[test]
    fn special_is_gated_by_cooldown() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(SPECIAL_MIN_RANGE + 1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Special));

        // Still within cooldown - immediately re-casting should fail.
        assert!(!m.apply_attack(Player::P1, Attack::Special));

        // Exactly at the cooldown boundary - still not ready.
        m.tick += SPECIAL_COOLDOWN_TICKS - 1;
        assert!(!m.apply_attack(Player::P1, Attack::Special));

        // One tick further - cooldown has elapsed.
        m.tick += 1;
        assert!(m.apply_attack(Player::P1, Attack::Special));
    }

    #[test]
    fn special_is_gated_by_recent_damage_lockout() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(SPECIAL_MIN_RANGE + 1.0, Facing::Left);

        // P1 gets hit by P2's Punch (bring them into Punch range briefly).
        m.p1.position = 0.0;
        m.p2.position = 1.0;
        assert!(m.apply_attack(Player::P2, Attack::Punch));
        assert_eq!(m.p1.last_hit_tick, Some(0));

        // Move back out to Special range; P1 tries to cast immediately -
        // still within the lockout window.
        m.p2.position = SPECIAL_MIN_RANGE + 1.0;
        assert!(!m.apply_attack(Player::P1, Attack::Special));

        // Exactly at the lockout boundary - still not ready.
        m.tick += SPECIAL_LOCKOUT_TICKS - 1;
        assert!(!m.apply_attack(Player::P1, Attack::Special));

        // One tick further - lockout has elapsed.
        m.tick += 1;
        assert!(m.apply_attack(Player::P1, Attack::Special));
    }

    #[test]
    fn match_ends_exactly_when_health_reaches_zero() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);
        m.p2.health = 2;

        assert!(!m.has_ended());
        assert!(m.apply_attack(Player::P1, Attack::Punch));
        assert_eq!(m.p2.health, 1);
        assert!(!m.has_ended());

        assert!(m.apply_attack(Player::P1, Attack::Punch));
        assert_eq!(m.p2.health, 0);
        assert!(m.has_ended());
        assert_eq!(m.winner, Some(Player::P1));
    }

    #[test]
    fn move_player_applies_delta_in_either_direction() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);

        m.move_player(Player::P1, MOVE_SPEED_PER_TICK);
        assert_eq!(m.p1.position, MOVE_SPEED_PER_TICK);

        m.move_player(Player::P1, -2.0 * MOVE_SPEED_PER_TICK);
        assert_eq!(m.p1.position, -MOVE_SPEED_PER_TICK);
    }

    #[test]
    fn move_player_clamps_at_the_positive_stage_bound() {
        let mut m = MatchState::new();
        m.p1 = state_at(STAGE_HALF_WIDTH - 0.01, Facing::Right);

        m.move_player(Player::P1, 10.0);
        assert_eq!(m.p1.position, STAGE_HALF_WIDTH);
    }

    #[test]
    fn move_player_clamps_at_the_negative_stage_bound() {
        let mut m = MatchState::new();
        m.p1 = state_at(-STAGE_HALF_WIDTH + 0.01, Facing::Right);

        m.move_player(Player::P1, -10.0);
        assert_eq!(m.p1.position, -STAGE_HALF_WIDTH);
    }

    #[test]
    fn move_player_does_nothing_after_the_match_has_ended() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.winner = Some(Player::P2);

        m.move_player(Player::P1, 1.0);
        assert_eq!(m.p1.position, 0.0);
    }

    #[test]
    fn punch_starts_the_attack_animation_even_when_it_whiffs() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(100.0, Facing::Left); // far out of range

        assert!(!m.apply_attack(Player::P1, Attack::Punch));
        assert_eq!(m.p1.attack_animation, Some(Attack::Punch));
    }

    #[test]
    fn kick_starts_the_attack_animation_when_it_lands() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Kick));
        assert_eq!(m.p1.attack_animation, Some(Attack::Kick));
    }

    #[test]
    fn special_never_starts_an_attack_animation() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(SPECIAL_MIN_RANGE + 1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Special));
        assert_eq!(m.p1.attack_animation, None);
    }

    #[test]
    fn attack_animation_clears_after_its_duration() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        m.apply_attack(Player::P1, Attack::Punch);
        assert_eq!(m.p1.attack_animation, Some(Attack::Punch));

        for _ in 0..ATTACK_ANIMATION_TICKS - 1 {
            m.advance_tick();
            assert_eq!(m.p1.attack_animation, Some(Attack::Punch));
        }

        m.advance_tick();
        assert_eq!(m.p1.attack_animation, None);
    }

    #[test]
    fn a_second_attack_restarts_the_animation_duration() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        m.apply_attack(Player::P1, Attack::Punch);
        for _ in 0..ATTACK_ANIMATION_TICKS - 1 {
            m.advance_tick();
        }
        // One tick from clearing - a fresh attack now should restart the
        // full duration rather than clearing on the next tick anyway.
        m.apply_attack(Player::P1, Attack::Kick);
        m.advance_tick();
        assert_eq!(m.p1.attack_animation, Some(Attack::Kick));
    }
}
