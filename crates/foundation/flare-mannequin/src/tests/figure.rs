use super::close;
use crate::{Angles, Body, Joint, Mannequin, Pose, PoseError, Proportion, Proportions, Side, Vec3};

#[test]
fn the_joints_list_parents_first_and_every_name_reads_back() {
    for (i, joint) in Joint::ALL.into_iter().enumerate() {
        assert_eq!(joint.index(), i);
        if let Some(parent) = joint.parent() {
            assert!(parent.index() < i, "{joint:?} after its parent");
        }
        assert_eq!(Joint::from_id(joint.id()), Some(joint));
        assert_eq!(joint.opposite().opposite(), joint);
        assert_eq!(joint.opposite().limits(), joint.limits());
        assert!(
            joint.limits().contains(Angles::ZERO),
            "{joint:?} rests in range"
        );
    }
    assert_eq!(Joint::from_id("tail"), None);
}

#[test]
fn at_rest_the_figure_stands_one_unit_tall_on_the_ground_and_both_sides_match() {
    for body in Body::ALL {
        let m = Mannequin::new(body);
        let posed = m.posed();
        let top = posed.point(
            Joint::Head,
            Vec3::new(
                0.0,
                0.12 * if body == Body::Female { 1.04 } else { 1.0 },
                0.0,
            ),
        );
        assert!(close(top.y, 1.0, 0.03), "{body:?} is {}", top.y);
        assert!(close(posed.at(Joint::LeftAnkle).y, 0.04, 1e-9));
        for joint in Joint::ALL.into_iter().filter(|j| j.side() == Side::Left) {
            let (l, r) = (posed.at(joint), posed.at(joint.opposite()));
            assert!(l.x > 0.0, "the figure's left is +x");
            assert!(close(l.x, -r.x, 1e-12) && close(l.y, r.y, 1e-12) && close(l.z, r.z, 1e-12));
        }
    }
}

#[test]
fn bends_go_the_way_a_body_bends() {
    let mut m = Mannequin::default();
    m.pose.set(Joint::LeftElbow, Angles::new(-90.0, 0.0, 0.0));
    m.pose.set(Joint::LeftKnee, Angles::new(90.0, 0.0, 0.0));
    m.pose
        .set(Joint::RightShoulder, Angles::new(0.0, 0.0, 90.0));
    m.pose.set(Joint::Spine, Angles::new(30.0, 0.0, 0.0));
    let posed = m.posed();
    let (elbow, wrist) = (posed.at(Joint::LeftElbow), posed.at(Joint::LeftWrist));
    assert!(wrist.z - elbow.z > 0.1, "the forearm comes forward");
    let (knee, ankle) = (posed.at(Joint::LeftKnee), posed.at(Joint::LeftAnkle));
    assert!(ankle.z - knee.z < -0.1, "the shin goes back");
    let (shoulder, elbow) = (posed.at(Joint::RightShoulder), posed.at(Joint::RightElbow));
    assert!(
        elbow.x - shoulder.x < -0.15 && close(elbow.y, shoulder.y, 1e-9),
        "out to the side"
    );
    assert!(posed.at(Joint::Neck).z > 0.05, "the body leans forward");
}

#[test]
fn a_joint_is_held_to_its_limits_and_a_checked_one_is_refused() {
    let mut pose = Pose::default();
    pose.set(Joint::LeftKnee, Angles::new(-40.0, 0.0, 0.0));
    assert_eq!(
        pose.angles(Joint::LeftKnee),
        Angles::ZERO,
        "a knee does not bend forward"
    );
    pose.set(Joint::Pelvis, Angles::new(0.0, 200.0, 0.0));
    assert_eq!(
        pose.angles(Joint::Pelvis).twist,
        -160.0,
        "the whole figure turns round"
    );
    pose.set(Joint::LeftElbow, Angles::new(f64::NAN, 0.0, 0.0));
    assert_eq!(
        pose.angles(Joint::LeftElbow),
        Angles::ZERO,
        "not a number changes nothing"
    );
    let refused = pose.set_checked(Joint::RightElbow, Angles::new(20.0, 0.0, 0.0));
    assert!(matches!(
        refused,
        Err(PoseError::OutOfRange {
            joint: "right_elbow",
            axis: "bend",
            ..
        })
    ));
    assert!(refused.unwrap_err().to_string().contains("-150"));
    assert!(pose.set_lift_checked(Vec3::new(0.0, 3.0, 0.0)).is_err());
    pose.set_lift(Vec3::new(0.0, 3.0, -9.0));
    assert_eq!(pose.lift(), Vec3::new(0.0, 2.0, -2.0));
}

#[test]
fn a_mirrored_pose_is_the_mirror_image_of_the_figure() {
    let mut m = Mannequin::new(Body::Female);
    m.pose = crate::preset("wave").expect("wave");
    m.pose.set(Joint::Chest, Angles::new(10.0, 20.0, -15.0));
    m.pose.set(Joint::Pelvis, Angles::new(5.0, 30.0, 8.0));
    m.pose.set_lift(Vec3::new(0.2, 0.1, 0.0));
    let mut mirror = m;
    mirror.pose = m.pose.mirrored();
    let (a, b) = (m.posed(), mirror.posed());
    for joint in Joint::ALL {
        let (p, q) = (a.at(joint), b.at(joint.opposite()));
        assert!(
            close(p.x, -q.x, 1e-9) && close(p.y, q.y, 1e-9) && close(p.z, q.z, 1e-9),
            "{joint:?}"
        );
    }
    assert_eq!(m.pose.mirrored().mirrored(), m.pose);
}

#[test]
fn one_side_can_be_copied_to_the_other() {
    let mut pose = Pose::default();
    pose.set(Joint::LeftShoulder, Angles::new(-30.0, 10.0, 40.0));
    pose.set(Joint::RightKnee, Angles::new(60.0, 0.0, 0.0));
    pose.copy_side(Side::Left);
    assert_eq!(
        pose.angles(Joint::RightShoulder),
        Angles::new(-30.0, 10.0, 40.0)
    );
    assert_eq!(pose.angles(Joint::RightKnee), Angles::ZERO);
}

#[test]
fn proportions_are_held_to_their_ranges_and_change_the_figure() {
    let mut p = Proportions::default();
    assert!(p.set(Proportion::Legs, 2.0).is_err());
    assert!(p.set(Proportion::Head, f64::INFINITY).is_err());
    p.set_clamped(Proportion::Legs, 2.0);
    assert_eq!(p.get(Proportion::Legs), 1.25);
    let long = Mannequin {
        proportions: p,
        ..Mannequin::default()
    };
    let (short, long) = (Mannequin::default().posed(), long.posed());
    assert!(long.at(Joint::Pelvis).y > short.at(Joint::Pelvis).y + 0.1);
    assert!(
        close(long.at(Joint::LeftAnkle).y, 0.04, 1e-9),
        "still on the ground"
    );
}

#[test]
fn a_woman_has_narrower_shoulders_and_wider_hips() {
    let (m, f) = (
        Mannequin::new(Body::Male).posed(),
        Mannequin::new(Body::Female).posed(),
    );
    assert!(f.at(Joint::LeftShoulder).x < m.at(Joint::LeftShoulder).x);
    assert!(f.at(Joint::LeftHip).x > m.at(Joint::LeftHip).x);
}
