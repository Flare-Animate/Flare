use emath::Pos2;

use super::constrain;

const FROM: Pos2 = Pos2::new(100.0, 50.0);

#[test]
fn a_nearly_level_drag_becomes_exactly_level_either_way() {
    assert_eq!(
        constrain(FROM, Pos2::new(180.0, 62.0)),
        Pos2::new(180.0, 50.0)
    );
    assert_eq!(
        constrain(FROM, Pos2::new(20.0, 41.0)),
        Pos2::new(20.0, 50.0)
    );
}

#[test]
fn a_nearly_upright_drag_becomes_exactly_upright_either_way() {
    assert_eq!(
        constrain(FROM, Pos2::new(109.0, 130.0)),
        Pos2::new(100.0, 130.0)
    );
    assert_eq!(
        constrain(FROM, Pos2::new(93.0, -10.0)),
        Pos2::new(100.0, -10.0)
    );
}

#[test]
fn a_drag_near_a_diagonal_moves_the_same_on_both_axes() {
    // One for each of the four diagonals.
    for (to, (sx, sy)) in [
        (Pos2::new(160.0, 100.0), (1.0, 1.0)),
        (Pos2::new(40.0, 100.0), (-1.0, 1.0)),
        (Pos2::new(40.0, 0.0), (-1.0, -1.0)),
        (Pos2::new(160.0, 0.0), (1.0, -1.0)),
    ] {
        let end = constrain(FROM, to);
        let (dx, dy) = (end.x - FROM.x, end.y - FROM.y);
        assert!((dx.abs() - dy.abs()).abs() < 1e-3, "{to:?} gave {end:?}");
        assert_eq!((dx.signum(), dy.signum()), (sx, sy), "{to:?} gave {end:?}");
        // The pointer was 60 across and 50 down, so its nearest point on the diagonal is 55, 55.
        assert!((dx.abs() - 55.0).abs() < 1e-3, "{to:?} gave {end:?}");
    }
}

#[test]
fn the_direction_changes_half_way_between_two_of_them() {
    // 22.5 degrees is the boundary: 20 degrees is level, 25 is the diagonal.
    let at = |degrees: f32| {
        let r = degrees.to_radians();
        constrain(Pos2::ZERO, Pos2::new(100.0 * r.cos(), 100.0 * r.sin()))
    };
    assert_eq!(at(20.0).y, 0.0);
    let diagonal = at(25.0);
    assert!((diagonal.x - diagonal.y).abs() < 1e-3 && diagonal.y > 1.0);
}

#[test]
fn a_point_already_on_a_direction_stays_where_it_is() {
    for to in [
        Pos2::new(170.0, 50.0),
        Pos2::new(100.0, -20.0),
        Pos2::new(130.0, 80.0),
    ] {
        let end = constrain(FROM, to);
        assert!((end - to).length() < 1e-3, "{to:?} gave {end:?}");
    }
}

#[test]
fn no_drag_and_points_that_are_not_finite_are_left_alone() {
    assert_eq!(constrain(FROM, FROM), FROM);
    let nan = Pos2::new(f32::NAN, 10.0);
    assert!(constrain(FROM, nan).x.is_nan());
    let far = Pos2::new(f32::INFINITY, 10.0);
    assert_eq!(constrain(FROM, far), far);
    assert_eq!(constrain(far, FROM), FROM);
}
