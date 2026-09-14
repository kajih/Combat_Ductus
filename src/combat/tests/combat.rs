use super::*;

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
