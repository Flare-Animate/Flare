use proptest::prelude::*;

use super::close;
use crate::math::wrap_degrees;
use crate::{Angles, Rot3, Vec3};

fn same(a: Rot3, b: Rot3) -> bool {
    a.0.iter()
        .flatten()
        .zip(b.0.iter().flatten())
        .all(|(x, y)| close(*x, *y, 1e-9))
}

#[test]
fn a_turn_about_an_axis_is_the_turn_about_that_axis() {
    assert!(same(
        Rot3::about_axis(Vec3::new(1.0, 0.0, 0.0), 37.0),
        Rot3::about_x(37.0)
    ));
    assert!(same(
        Rot3::about_axis(Vec3::new(0.0, 1.0, 0.0), -12.0),
        Rot3::about_y(-12.0)
    ));
    assert!(same(
        Rot3::about_axis(Vec3::new(0.0, 0.0, 1.0), 150.0),
        Rot3::about_z(150.0)
    ));
    let r = Rot3::about_axis(Vec3::new(0.6, 0.0, 0.8), 71.0);
    assert!(same(r * r.transpose(), Rot3::IDENTITY), "orthonormal");
}

#[test]
fn a_twist_never_moves_where_the_bone_points() {
    let bone = Vec3::new(0.0, -1.0, 0.0);
    let plain = Rot3::from_angles(Angles::new(-40.0, 0.0, 30.0)).apply(bone);
    let twisted = Rot3::from_angles(Angles::new(-40.0, 75.0, 30.0)).apply(bone);
    assert!((plain - twisted).length() < 1e-12);
}

#[test]
fn a_quarter_bend_comes_back_with_the_twist_folded_into_the_swing() {
    let r = Rot3::from_angles(Angles::new(90.0, 20.0, 30.0));
    let back = r.to_angles();
    assert!(close(back.bend, 90.0, 1e-6) && back.twist == 0.0);
    assert!(same(Rot3::from_angles(back), r));
}

#[test]
fn a_share_of_a_turn_turns_that_far_about_the_same_axis() {
    let axis = Vec3::new(0.6, 0.0, 0.8);
    let r = Rot3::about_axis(axis, 120.0);
    assert!(same(r.partial(0.5), Rot3::about_axis(axis, 60.0)));
    assert!(same(r.partial(1.0), r));
    assert!(same(r.partial(0.0), Rot3::IDENTITY));
    assert!(same(Rot3::IDENTITY.partial(0.5), Rot3::IDENTITY));
    // A half turn, where reading the axis back is least steady.
    let half = Rot3::about_axis(axis, 180.0);
    assert!(same(half.partial(0.5) * half.partial(0.5), half));
}

#[test]
fn degrees_wrap_into_a_half_turn_either_way() {
    assert_eq!(wrap_degrees(190.0), -170.0);
    assert_eq!(wrap_degrees(-180.0), 180.0);
    assert_eq!(wrap_degrees(540.0), 180.0);
    assert_eq!(wrap_degrees(-30.0), -30.0);
}

proptest! {
    #[test]
    fn two_halves_of_a_turn_make_the_turn(
        bend in -180.0f64..180.0,
        twist in -180.0f64..180.0,
        side in -180.0f64..180.0,
    ) {
        let r = Rot3::from_angles(Angles::new(bend, twist, side));
        let half = r.partial(0.5);
        prop_assert!(same(half * half, r));
    }

    #[test]
    fn angles_come_back_from_their_rotation(
        bend in -89.0f64..89.0,
        twist in -179.0f64..179.0,
        side in -179.0f64..179.0,
    ) {
        let back = Rot3::from_angles(Angles::new(bend, twist, side)).to_angles();
        prop_assert!(close(back.bend, bend, 1e-7));
        prop_assert!(close(back.twist, twist, 1e-7));
        prop_assert!(close(back.side, side, 1e-7));
    }
}
