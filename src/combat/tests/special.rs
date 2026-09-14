use super::*;

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
fn special_never_starts_an_attack_animation() {
    let mut m = MatchState::new();
    m.p1 = state_at(0.0, Facing::Right);
    m.p2 = state_at(SPECIAL_MIN_RANGE + 1.0, Facing::Left);

    assert!(m.apply_attack(Player::P1, Attack::Special));
    assert_eq!(m.p1.attack_animation, None);
}
