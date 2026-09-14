use super::*;

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
