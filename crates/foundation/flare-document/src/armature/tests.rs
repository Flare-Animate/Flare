#![allow(clippy::expect_used)]

use flare_core::{InstanceId, SymbolId, Transform2D};
use std::f64::consts::FRAC_PI_2;

use super::*;

fn point(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}

fn instance_at(x: f64, y: f64) -> Instance {
    Instance::new(
        InstanceId::new(),
        SymbolId::new(),
        Transform2D::new(DAffine2::from_translation(DVec2::new(x, y))).expect("finite"),
    )
}

/// An arm along the x axis: a shoulder at 0, an elbow at 10 and a hand at 20.
fn arm() -> Armature {
    Armature::new(vec![
        Joint::new(instance_at(0.0, 0.0), point(0.0, 0.0), None),
        Joint::new(instance_at(10.0, 0.0), point(10.0, 0.0), Some(0)),
        Joint::new(instance_at(20.0, 0.0), point(20.0, 0.0), Some(1)),
    ])
    .expect("arm")
}

fn close(a: DVec2, b: DVec2) -> bool {
    (a - b).length() < 1e-9
}

fn turned(armature: &Armature, turns: [f64; 3]) -> Armature {
    armature
        .with_pose(
            turns
                .iter()
                .map(|&turn| JointPose {
                    turn,
                    shift: DVec2::ZERO,
                })
                .collect(),
        )
        .expect("pose")
}

#[test]
fn at_rest_every_instance_is_where_it_joined() {
    let arm = arm();
    for (placed, joint) in arm.placements().iter().zip(arm.joints()) {
        assert_eq!(*placed, joint.instance().transform().as_affine());
    }
}

#[test]
fn turning_a_joint_carries_every_bone_below_it() {
    // A quarter turn at the shoulder points the arm down (y grows downward, clockwise is +).
    let posed = turned(&arm(), [FRAC_PI_2, 0.0, 0.0]).posed();
    assert!(close(posed[1].0, DVec2::new(0.0, 10.0)));
    assert!(close(posed[2].0, DVec2::new(0.0, 20.0)));
    // Then the elbow turns back: the hand points along x again, from the elbow.
    let posed = turned(&arm(), [FRAC_PI_2, -FRAC_PI_2, 0.0]).posed();
    assert!(close(posed[2].0, DVec2::new(10.0, 10.0)));
    assert!((posed[2].1).abs() < 1e-12);
}

#[test]
fn an_instance_is_drawn_through_its_joint() {
    let arm = turned(&arm(), [FRAC_PI_2, 0.0, 0.0]);
    let hand = arm.placements()[2];
    // The hand's own origin was at its joint, so it goes where the joint went, turned.
    assert!(close(hand.translation, DVec2::new(0.0, 20.0)));
    assert!(close(
        hand.transform_vector2(DVec2::X),
        DVec2::new(0.0, 1.0)
    ));
}

#[test]
fn a_root_moves_by_its_shift_and_a_child_cannot() {
    let arm = arm();
    let mut pose = vec![JointPose::default(); 3];
    pose[0].shift = DVec2::new(5.0, 5.0);
    let moved = arm.with_pose(pose.clone()).expect("root shift");
    assert!(close(moved.posed()[2].0, DVec2::new(25.0, 5.0)));
    pose[1].shift = DVec2::new(1.0, 0.0);
    assert_eq!(arm.with_pose(pose), Err(ArmatureError::ShiftOnChild(1)));
}

#[test]
fn rigs_that_cannot_be_posed_are_refused() {
    assert_eq!(Armature::new(vec![]), Err(ArmatureError::Empty));
    let loop_back = Joint::new(instance_at(0.0, 0.0), point(0.0, 0.0), Some(0));
    assert_eq!(
        Armature::new(vec![loop_back]),
        Err(ArmatureError::ParentNotBefore(0))
    );
    let one = instance_at(0.0, 0.0);
    let twice = vec![
        Joint::new(one.clone(), point(0.0, 0.0), None),
        Joint::new(one, point(1.0, 0.0), Some(0)),
    ];
    assert_eq!(
        Armature::new(twice),
        Err(ArmatureError::DuplicateInstance(1))
    );
    let many = (0..=MAX_JOINTS)
        .map(|i| Joint::new(instance_at(0.0, 0.0), point(i as f64, 0.0), None))
        .collect();
    assert_eq!(
        Armature::new(many),
        Err(ArmatureError::TooManyJoints(MAX_JOINTS + 1))
    );
    assert_eq!(
        arm().with_pose(vec![JointPose::default()]),
        Err(ArmatureError::PoseLength(3))
    );
    let mut pose = vec![JointPose::default(); 3];
    pose[2].turn = f64::NAN;
    assert_eq!(arm().with_pose(pose), Err(ArmatureError::NotFinite(2)));
}

#[test]
fn limits_are_checked_and_keep_a_turn_inside_them() {
    let joint = Joint::new(instance_at(0.0, 0.0), point(0.0, 0.0), None);
    assert_eq!(
        joint.clone().with_limit(Some((1.0, -1.0))),
        Err(ArmatureError::Limit)
    );
    assert_eq!(
        joint.clone().with_limit(Some((f64::NAN, 1.0))),
        Err(ArmatureError::Limit)
    );
    let limited = joint.with_limit(Some((-0.5, 0.25))).expect("limit");
    assert_eq!(limited.limited(1.0), 0.25);
    assert_eq!(limited.limited(-1.0), -0.5);
    assert_eq!(limited.limited(0.1), 0.1);
}

#[test]
fn a_pose_tweens_to_another_of_the_same_rig_only() {
    let from = arm();
    let to = turned(&from, [1.0, -0.5, 0.0]);
    let half = from.lerp(&to, 0.5).expect("same rig");
    assert!((half.pose()[0].turn - 0.5).abs() < 1e-12);
    assert!((half.pose()[1].turn + 0.25).abs() < 1e-12);
    let other = arm();
    assert!(from.lerp(&other, 0.5).is_none(), "other instances");
}

#[test]
fn chains_and_children_follow_the_parents() {
    let arm = arm();
    assert_eq!(arm.chain(2), vec![2, 1, 0]);
    assert_eq!(arm.chain(9), Vec::<usize>::new());
    assert_eq!(arm.children(0).collect::<Vec<_>>(), vec![1]);
    let hand = arm.joints()[2].instance().id();
    assert_eq!(arm.joint_of(hand), Some(2));
}

#[test]
fn detaching_a_bone_keeps_everything_where_it_is() {
    let posed = turned(&arm(), [FRAC_PI_2, 0.3, -0.2]);
    let before = posed.placements();
    let detached = posed.edited(&RigEdit::Detach(1)).expect("detach");
    assert_eq!(detached.joints()[1].parent(), None);
    assert_eq!(
        detached.joints()[2].parent(),
        None,
        "the bones below go too"
    );
    assert_eq!(detached.joints()[0].parent(), None);
    let limited = posed
        .edited(&RigEdit::Limit(2, Some((-0.5, 0.5))))
        .expect("limit")
        .edited(&RigEdit::Detach(1))
        .expect("detach");
    assert_eq!(
        limited.joints()[2].limit(),
        None,
        "the limit went with its bone"
    );
    for (a, b) in before.iter().zip(detached.placements()) {
        assert!(close(a.translation, b.translation));
        assert!(close(a.matrix2.x_axis, b.matrix2.x_axis));
    }
}

#[test]
fn a_joint_added_below_a_posed_one_stays_where_it_was_drawn() {
    let posed = turned(&arm(), [FRAC_PI_2, 0.0, 0.0]);
    let placed = DAffine2::from_translation(DVec2::new(3.0, 30.0));
    let (rest, at) = posed
        .rest_below(2, placed, DVec2::new(0.0, 30.0))
        .expect("parent");
    let instance = Instance::new(InstanceId::new(), SymbolId::new(), rest);
    let grown = posed
        .edited(&RigEdit::Add(Joint::new(instance, at, Some(2))))
        .expect("add");
    assert!(close(grown.placements()[3].translation, placed.translation));
    assert!(close(grown.posed()[3].0, DVec2::new(0.0, 30.0)));
}

#[test]
fn a_limit_brings_each_pose_inside_it() {
    let posed = turned(&arm(), [0.0, 1.0, 0.0]);
    let limited = posed
        .edited(&RigEdit::Limit(1, Some((-0.5, 0.5))))
        .expect("limit");
    assert_eq!(limited.pose()[1].turn, 0.5);
    assert!(posed.edited(&RigEdit::Limit(7, None)).is_err());
}
