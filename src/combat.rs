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

/// Ticks between successive Kicks, measured from either Character's last
/// Punch *or* Kick (a shared cooldown clock, not a per-attack one - see
/// `CharacterState::last_attack_tick`). Placeholder tuning value; kept
/// longer than `ATTACK_ANIMATION_TICKS` so a Kick's own swing always
/// finishes well before it's usable again.
pub const KICK_COOLDOWN_TICKS: u64 = 16;
/// Ticks between successive Punches, measured the same way. Punch has a
/// shorter, steadier cadence than Kick - half of Kick's cooldown, not a
/// separate "two free punches" allowance.
pub const PUNCH_COOLDOWN_TICKS: u64 = KICK_COOLDOWN_TICKS / 2;

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

/// How long a Jump's full rise-and-fall arc lasts, in ticks, from the
/// instant it's triggered to landing again. Placeholder tuning value; ~0.8
/// second at the server's ~30Hz tick rate.
pub const JUMP_DURATION_TICKS: u8 = 24;
/// The peak height a Jump reaches, in the same abstract game-position units
/// as `CharacterState::position` (not pixels — rendering scales this up).
/// Placeholder tuning value.
pub const JUMP_HEIGHT: f32 = 0.8;

/// How long a successfully-cast Motivational Speech's speech-bubble VFX
/// stays up, in ticks - long enough to actually read a short line of text,
/// noticeably longer than `ATTACK_ANIMATION_TICKS`. Placeholder tuning
/// value; ~2 seconds at the server's ~30Hz tick rate.
pub const SPEECH_BUBBLE_DURATION_TICKS: u8 = 60;

/// One Character's simulated state within a Match.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CharacterState {
    pub position: f32,
    pub facing: Facing,
    pub health: u8,
    pub airborne: bool,
    /// How high off the ground a Character currently is, in the same
    /// abstract game-position units as `position`. Zero whenever `airborne`
    /// is false; driven by `tick_jump` through a simple rise-and-fall arc
    /// while it's true. Rendering-only - no combat rule depends on the
    /// actual height, only on `airborne` itself.
    pub vertical_offset: f32,
    jump_ticks_remaining: u8,
    /// Whether a successfully-cast Motivational Speech's speech-bubble VFX
    /// is currently showing. Only ever set by a *successful* Special cast -
    /// unlike Punch/Kick, a rejected/gated Special has no feedback at all
    /// (see `attack_animation`'s doc comment and the existing
    /// `special_never_starts_an_attack_animation` test).
    pub speaking: bool,
    speech_bubble_ticks_remaining: u8,
    /// The tick Special was last successfully cast, if ever. Cooldown is
    /// measured from this.
    pub special_last_cast_tick: Option<u64>,
    /// The tick this Character last threw a Punch *or* Kick, if ever - one
    /// shared timestamp for both, updated by whichever was thrown most
    /// recently (regardless of whether it landed). Punch/Kick cooldown is
    /// measured from this (see `KICK_COOLDOWN_TICKS`/`PUNCH_COOLDOWN_TICKS`
    /// and `attack_cooldown_elapsed`).
    pub last_attack_tick: Option<u64>,
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
            vertical_offset: 0.0,
            jump_ticks_remaining: 0,
            speaking: false,
            speech_bubble_ticks_remaining: 0,
            special_last_cast_tick: None,
            last_attack_tick: None,
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

    fn start_jump(&mut self) {
        self.airborne = true;
        self.jump_ticks_remaining = JUMP_DURATION_TICKS;
    }

    /// Advances the jump arc by one tick, if one is in progress. Uses a
    /// simple parabola (zero at takeoff and landing, `JUMP_HEIGHT` at the
    /// midpoint) rather than any real gravity simulation - Jump is purely a
    /// repositioning tool, not a physics feature.
    fn tick_jump(&mut self) {
        if self.jump_ticks_remaining == 0 {
            return;
        }

        self.jump_ticks_remaining -= 1;

        if self.jump_ticks_remaining == 0 {
            self.airborne = false;
            self.vertical_offset = 0.0;
            return;
        }

        let elapsed = JUMP_DURATION_TICKS - self.jump_ticks_remaining;
        let progress = elapsed as f32 / JUMP_DURATION_TICKS as f32;
        self.vertical_offset = JUMP_HEIGHT * 4.0 * progress * (1.0 - progress);
    }

    fn start_speech_bubble(&mut self) {
        self.speaking = true;
        self.speech_bubble_ticks_remaining = SPEECH_BUBBLE_DURATION_TICKS;
    }

    fn tick_speech_bubble(&mut self) {
        if self.speech_bubble_ticks_remaining > 0 {
            self.speech_bubble_ticks_remaining -= 1;
            if self.speech_bubble_ticks_remaining == 0 {
                self.speaking = false;
            }
        }
    }
}

/// Why a Match ended, alongside who won (`MatchState::winner`) — a health
/// depleted win reads very differently to the loser than one they never
/// actually got to fight for. See
/// `docs/issues/combat-foundation/forfeit-win-on-disconnect.md`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchEndReason {
    /// The winner depleted the loser's Health via a landed Punch, Kick, or
    /// Special.
    Defeated,
    /// The loser's connection disconnected mid-Match while the winner's
    /// connection was still present — see `MatchState::forfeit`.
    Forfeit,
}

/// The full simulated state of one Match, and the single source of truth
/// the server steps every tick (ADR 0005 — the client never simulates).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchState {
    pub tick: u64,
    pub p1: CharacterState,
    pub p2: CharacterState,
    /// `None` while the Match is in progress; set the instant a Character's
    /// Health reaches 0, or a forfeit is declared (`forfeit`). Terminal — a
    /// Match is never restarted in v1.
    pub winner: Option<Player>,
    /// `None` exactly when `winner` is `None` — set alongside it, always,
    /// by whichever of `apply_attack`/`forfeit` actually decided the Match.
    pub end_reason: Option<MatchEndReason>,
}

impl MatchState {
    pub fn new() -> Self {
        MatchState {
            tick: 0,
            p1: CharacterState::new(-ATTACK_RANGE * 2.0, Facing::Right),
            p2: CharacterState::new(ATTACK_RANGE * 2.0, Facing::Left),
            winner: None,
            end_reason: None,
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
        self.p1.tick_jump();
        self.p2.tick_jump();
        self.p1.tick_speech_bubble();
        self.p2.tick_speech_bubble();
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

    /// Sends `player` airborne on a Jump arc (Space) — purely vertical
    /// repositioning, driven entirely by `tick_jump` from here on. A no-op
    /// once the Match has ended, or if the Character is already airborne
    /// (no double-jump).
    pub fn jump(&mut self, player: Player) {
        if self.has_ended() {
            return;
        }

        let character = self.character_mut(player);
        if character.airborne {
            return;
        }

        character.start_jump();
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

        // A cooldown-gated Punch/Kick is silently dropped - no animation,
        // no damage, no feedback of any kind (unlike an out-of-range whiff,
        // which still plays the swing). Checked before anything else so a
        // gated attempt never touches `last_attack_tick` or the animation.
        if matches!(attack, Attack::Punch | Attack::Kick)
            && !attack_cooldown_elapsed(self.character(attacker), attack, tick)
        {
            return false;
        }

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
        // issue). Either attack also updates the shared cooldown clock
        // here, on landing or whiffing alike - a thrown attack still has
        // recovery, even one that misses.
        if matches!(attack, Attack::Punch | Attack::Kick) {
            let attacker_state = self.character_mut(attacker);
            attacker_state.start_attack_animation(attack);
            attacker_state.last_attack_tick = Some(tick);
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
            let attacker_state = self.character_mut(attacker);
            attacker_state.special_last_cast_tick = Some(tick);
            attacker_state.start_speech_bubble();
        }

        let defender_state = self.character_mut(defender);
        defender_state.health = defender_state.health.saturating_sub(damage);
        defender_state.last_hit_tick = Some(tick);

        if defender_state.health == 0 {
            self.winner = Some(attacker);
            self.end_reason = Some(MatchEndReason::Defeated);
        }

        true
    }

    /// Directly declares the winner of a forfeit: `disconnected`'s
    /// connection just ended mid-Match while their opponent's connection
    /// was still present, so the opponent wins without landing a hit. A
    /// no-op if the Match has already ended, whichever way — a forfeit can
    /// never overwrite an already-decided Match, the same guard every other
    /// state-mutating method here already respects via `has_ended()`.
    ///
    /// Deciding *whether* a disconnect actually warrants a forfeit (both
    /// slots held by real connected clients at the moment of disconnect,
    /// not one of them the stationary Idle Opponent) is `server_net`'s job,
    /// not this method's — by the time this is called, that's already
    /// decided. This only ever decides *who won*, given *who left*.
    pub fn forfeit(&mut self, disconnected: Player) {
        if self.has_ended() {
            return;
        }

        self.winner = Some(disconnected.opponent());
        self.end_reason = Some(MatchEndReason::Forfeit);
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

/// Whether enough ticks have passed since `attacker`'s last Punch *or* Kick
/// for a new `attack` (Punch or Kick) to be thrown. Kick requires the full
/// `KICK_COOLDOWN_TICKS`; Punch only half that - either attack gates the
/// other, since both read from the same shared `last_attack_tick`.
fn attack_cooldown_elapsed(attacker: &CharacterState, attack: Attack, tick: u64) -> bool {
    let threshold = match attack {
        Attack::Punch => PUNCH_COOLDOWN_TICKS,
        Attack::Kick => KICK_COOLDOWN_TICKS,
        Attack::Special => return true, // Special has its own, separate cooldown.
    };

    match attacker.last_attack_tick {
        Some(last) => tick.saturating_sub(last) >= threshold,
        None => true,
    }
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
        assert!(m.p1.speaking);
    }

    #[test]
    fn special_fails_exactly_at_min_range_boundary() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(SPECIAL_MIN_RANGE, Facing::Left);

        assert!(!m.apply_attack(Player::P1, Attack::Special));
        assert!(!m.p1.speaking);
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
    fn rejected_special_casts_never_show_a_speech_bubble() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);

        // Too close - gated by minimum range.
        m.p2 = state_at(SPECIAL_MIN_RANGE, Facing::Left);
        assert!(!m.apply_attack(Player::P1, Attack::Special));
        assert!(!m.p1.speaking);

        // Far enough now, but still within cooldown from a prior cast.
        m.p2 = state_at(SPECIAL_MIN_RANGE + 1.0, Facing::Left);
        assert!(m.apply_attack(Player::P1, Attack::Special));
        m.p1.speaking = false; // reset to isolate the next (rejected) cast
        assert!(!m.apply_attack(Player::P1, Attack::Special));
        assert!(!m.p1.speaking);
    }

    #[test]
    fn speech_bubble_clears_after_its_duration() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(SPECIAL_MIN_RANGE + 1.0, Facing::Left);

        m.apply_attack(Player::P1, Attack::Special);
        assert!(m.p1.speaking);

        for _ in 0..SPEECH_BUBBLE_DURATION_TICKS - 1 {
            m.advance_tick();
            assert!(m.p1.speaking);
        }

        m.advance_tick();
        assert!(!m.p1.speaking);
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

        // A second Punch has to wait out the cooldown from the first.
        m.tick += PUNCH_COOLDOWN_TICKS;
        assert!(m.apply_attack(Player::P1, Attack::Punch));
        assert_eq!(m.p2.health, 0);
        assert!(m.has_ended());
        assert_eq!(m.winner, Some(Player::P1));
        assert_eq!(m.end_reason, Some(MatchEndReason::Defeated));
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
    fn jump_sends_a_character_airborne_and_they_land_again() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);

        m.jump(Player::P1);
        assert!(m.p1.airborne);
        assert_eq!(m.p1.vertical_offset, 0.0);

        for _ in 0..JUMP_DURATION_TICKS - 1 {
            m.advance_tick();
            assert!(m.p1.airborne);
            assert!(m.p1.vertical_offset > 0.0);
        }

        m.advance_tick();
        assert!(!m.p1.airborne);
        assert_eq!(m.p1.vertical_offset, 0.0);
    }

    #[test]
    fn jump_arc_peaks_at_the_midpoint_and_is_symmetric() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.jump(Player::P1);

        let half = JUMP_DURATION_TICKS / 2;
        for _ in 0..half {
            m.advance_tick();
        }
        let midpoint_height = m.p1.vertical_offset;
        assert!((midpoint_height - JUMP_HEIGHT).abs() < 0.01);

        // One tick before takeoff and one tick before landing should read
        // the same height - the arc is symmetric, not a sawtooth.
        let mut early = MatchState::new();
        early.p1 = state_at(0.0, Facing::Right);
        early.jump(Player::P1);
        early.advance_tick();
        let mut late = MatchState::new();
        late.p1 = state_at(0.0, Facing::Right);
        late.jump(Player::P1);
        for _ in 0..JUMP_DURATION_TICKS - 1 {
            late.advance_tick();
        }
        assert!((early.p1.vertical_offset - late.p1.vertical_offset).abs() < 0.01);
    }

    #[test]
    fn jump_does_not_change_horizontal_position() {
        let mut m = MatchState::new();
        m.p1 = state_at(1.0, Facing::Right);

        m.jump(Player::P1);
        for _ in 0..JUMP_DURATION_TICKS {
            m.advance_tick();
        }

        assert_eq!(m.p1.position, 1.0);
    }

    #[test]
    fn jumping_again_while_already_airborne_is_a_no_op() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);

        m.jump(Player::P1);
        m.advance_tick();
        let mid_arc_remaining_offset = m.p1.vertical_offset;

        // A second Jump input mid-arc shouldn't restart the arc's duration.
        m.jump(Player::P1);
        m.advance_tick();
        assert!(m.p1.vertical_offset != mid_arc_remaining_offset);
        for _ in 0..JUMP_DURATION_TICKS - 2 {
            m.advance_tick();
        }
        assert!(!m.p1.airborne);
    }

    #[test]
    fn jump_does_nothing_after_the_match_has_ended() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.winner = Some(Player::P2);

        m.jump(Player::P1);
        assert!(!m.p1.airborne);
    }

    #[test]
    fn a_second_attack_restarts_the_animation_duration() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        // A Kick first, so the second attack (a Punch, needing only half
        // Kick's cooldown) is already off cooldown by the time it's thrown
        // below - it's the animation restart being tested here, not the
        // cooldown gate.
        m.apply_attack(Player::P1, Attack::Kick);
        for _ in 0..ATTACK_ANIMATION_TICKS - 1 {
            m.advance_tick();
        }
        // One tick from clearing - a fresh attack now should restart the
        // full duration rather than clearing on the next tick anyway.
        assert!(m.apply_attack(Player::P1, Attack::Punch));
        m.advance_tick();
        assert_eq!(m.p1.attack_animation, Some(Attack::Punch));
    }

    #[test]
    fn kick_requires_a_full_cooldown_since_the_last_punch_or_kick() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Punch));

        // Barely any time has passed - Kick is gated.
        assert!(!m.apply_attack(Player::P1, Attack::Kick));

        // Exactly at Kick's full-cooldown boundary since the Punch - still
        // not ready.
        m.tick += KICK_COOLDOWN_TICKS - 1;
        assert!(!m.apply_attack(Player::P1, Attack::Kick));

        // One tick further - Kick's cooldown has elapsed.
        m.tick += 1;
        assert!(m.apply_attack(Player::P1, Attack::Kick));
    }

    #[test]
    fn punch_requires_only_half_the_kick_cooldown_since_the_last_punch_or_kick() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Punch));

        assert!(!m.apply_attack(Player::P1, Attack::Punch));

        m.tick += PUNCH_COOLDOWN_TICKS - 1;
        assert!(!m.apply_attack(Player::P1, Attack::Punch));

        m.tick += 1;
        assert!(m.apply_attack(Player::P1, Attack::Punch));
    }

    #[test]
    fn a_kick_delays_the_next_punch_by_half_the_kick_cooldown() {
        // Either attack gates the other, per punch-kick-cooldown.md - a
        // Kick blocks a following Punch too, not just a following Kick.
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Kick));

        assert!(!m.apply_attack(Player::P1, Attack::Punch));

        m.tick += PUNCH_COOLDOWN_TICKS - 1;
        assert!(!m.apply_attack(Player::P1, Attack::Punch));

        m.tick += 1;
        assert!(m.apply_attack(Player::P1, Attack::Punch));
    }

    #[test]
    fn cooldown_gated_attack_starts_no_animation_and_deals_no_damage() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Punch));
        for _ in 0..ATTACK_ANIMATION_TICKS {
            m.advance_tick();
        }
        // The Punch's own swing has already cleared, well before Kick's
        // full cooldown has - a Kick thrown now should be a total no-op,
        // not even a whiff animation.
        assert!(m.p1.attack_animation.is_none());

        let health_before = m.p2.health;
        assert!(!m.apply_attack(Player::P1, Attack::Kick));
        assert!(m.p1.attack_animation.is_none());
        assert_eq!(m.p2.health, health_before);
    }

    #[test]
    fn attack_cooldown_resets_on_a_fresh_match_state() {
        // Mirrors how a Match restart works in practice
        // (`InputEvent::RequestRestart` replaces the whole `MatchState`) -
        // confirmed here directly against `CharacterState`, with no extra
        // reset code involved.
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Kick));
        assert!(!m.apply_attack(Player::P1, Attack::Punch));

        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);

        assert!(m.apply_attack(Player::P1, Attack::Punch));
    }

    #[test]
    fn forfeit_declares_the_disconnected_players_opponent_the_winner() {
        let mut m = MatchState::new();

        m.forfeit(Player::P1);

        assert!(m.has_ended());
        assert_eq!(m.winner, Some(Player::P2));
        assert_eq!(m.end_reason, Some(MatchEndReason::Forfeit));
    }

    #[test]
    fn forfeit_does_not_overwrite_an_already_decided_match() {
        let mut m = MatchState::new();
        m.p1 = state_at(0.0, Facing::Right);
        m.p2 = state_at(1.0, Facing::Left);
        m.p2.health = 1;

        assert!(m.apply_attack(Player::P1, Attack::Punch));
        assert_eq!(m.winner, Some(Player::P1));
        assert_eq!(m.end_reason, Some(MatchEndReason::Defeated));

        // P2 (the loser) "disconnecting" after the fact should not flip the
        // Match into a forfeit win for P1 who, per this method's contract,
        // already won fair and square.
        m.forfeit(Player::P2);
        assert_eq!(m.winner, Some(Player::P1));
        assert_eq!(m.end_reason, Some(MatchEndReason::Defeated));
    }
}
