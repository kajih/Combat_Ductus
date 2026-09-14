//! Unit tests for `combat` - the sole carrier of unit tests in this crate
//! (see the parent module's doc comment): every rule here is exercised
//! directly, with no rendering or networking involved.

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
