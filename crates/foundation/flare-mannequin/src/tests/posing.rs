use super::{close, front, placement};
use crate::{Angles, Joint, Mannequin, View};

fn facing() -> Mannequin {
    Mannequin {
        view: front(),
        ..Mannequin::default()
    }
}

#[test]
fn a_drag_round_a_joint_turns_its_part_the_way_the_pointer_went() {
    let mut m = facing();
    let elbow = m.marks(placement())[Joint::LeftElbow.index()].at;
    let wrist = m.marks(placement())[Joint::LeftWrist.index()].at;
    // The forearm hangs straight down; drag its end out to the picture's right (the figure's left).
    m.turn(
        Joint::LeftElbow,
        placement(),
        wrist,
        [elbow[0] + 15.0, elbow[1]],
    );
    // An elbow only bends one way, so seen from the front it can only bend toward the viewer.
    let after = m.marks(placement())[Joint::LeftWrist.index()].at;
    assert!(after[1] <= wrist[1] + 1e-9);
    m.turn(
        Joint::LeftShoulder,
        placement(),
        m.marks(placement())[Joint::LeftElbow.index()].at,
        [
            elbow[0] + 40.0,
            m.marks(placement())[Joint::LeftShoulder.index()].at[1],
        ],
    );
    let angles = m.pose.angles(Joint::LeftShoulder);
    assert!(
        close(angles.side, 90.0, 0.5),
        "the arm is out to the side: {angles:?}"
    );
}

#[test]
fn an_arm_turned_on_round_over_the_head_keeps_going_instead_of_flipping() {
    let mut m = facing();
    for _ in 0..6 {
        // Counterclockwise as seen raises the figure's left arm (on the picture's right).
        m.turn_by(Joint::LeftShoulder, 28.0);
    }
    let angles = m.pose.angles(Joint::LeftShoulder);
    assert!(close(angles.side, 168.0, 0.5), "{angles:?}");
    assert!(close(angles.bend, 0.0, 0.5) && close(angles.twist, 0.0, 0.5));
    let posed = m.posed();
    assert!(posed.at(Joint::LeftWrist).y > posed.at(Joint::Head).y);
    // The right arm is mirrored: clockwise as seen raises it.
    m.turn_by(Joint::RightShoulder, -60.0);
    assert!(close(m.pose.angles(Joint::RightShoulder).side, 60.0, 0.5));
}

#[test]
fn a_turn_stops_at_the_joints_limit() {
    let mut m = facing();
    m.view = View::new(90.0, 0.0, 0.0);
    for _ in 0..10 {
        m.turn_by(Joint::LeftKnee, 30.0);
        m.turn_by(Joint::LeftKnee, -30.0);
    }
    let knee = m.pose.angles(Joint::LeftKnee);
    assert!(Joint::LeftKnee.limits().contains(knee), "{knee:?}");
    m.turn_by(Joint::Pelvis, 90.0);
    assert!(
        m.pose.angles(Joint::Pelvis) != Angles::ZERO,
        "the whole figure turns"
    );
}

#[test]
fn a_hand_reaches_a_point_it_can_get_to_and_points_at_one_it_cannot() {
    let mut m = facing();
    let shoulder = m.marks(placement())[Joint::LeftShoulder.index()].at;
    let target = [shoulder[0] + 20.0, shoulder[1] + 10.0];
    assert!(m.reach(Joint::LeftWrist, placement(), target));
    let wrist = m.marks(placement())[Joint::LeftWrist.index()].at;
    assert!((wrist[0] - target[0]).hypot(wrist[1] - target[1]) <= 0.25);
    for joint in Joint::ALL {
        assert!(joint == Joint::Pelvis || joint.limits().contains(m.pose.angles(joint)));
    }

    let far = [shoulder[0] + 1000.0, shoulder[1]];
    assert!(!m.reach(Joint::LeftWrist, placement(), far));
    let marks = m.marks(placement());
    let wrist = marks[Joint::LeftWrist.index()].at;
    assert!(wrist[0] > shoulder[0] + 25.0, "the arm stretches toward it");

    let mut foot = facing();
    let hip = foot.marks(placement())[Joint::RightHip.index()].at;
    assert!(foot.reach(
        Joint::RightAnkle,
        placement(),
        [hip[0] - 15.0, hip[1] + 30.0]
    ));

    assert!(
        !m.reach(Joint::LeftElbow, placement(), target),
        "only hands and feet reach"
    );
    assert!(!m.reach(Joint::LeftWrist, placement(), [f64::NAN, 0.0]));
}

#[test]
fn orbiting_turns_the_view_and_never_the_pose() {
    let mut m = facing();
    let pose = m.pose;
    m.orbit(200.0, 120.0);
    assert_eq!(m.view.yaw(), -160.0);
    assert_eq!(m.view.pitch(), 89.0);
    assert_eq!(m.pose, pose);
}
