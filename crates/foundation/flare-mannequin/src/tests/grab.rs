use super::{close, front, placement};
use crate::{Joint, Mannequin, Request, Vec3, WireError};

fn facing() -> Mannequin {
    Mannequin {
        view: front(),
        ..Mannequin::default()
    }
}

fn mark(m: &Mannequin, joint: Joint) -> [f64; 2] {
    m.marks(placement())[joint.index()].at
}

fn within_limits(m: &Mannequin) -> bool {
    Joint::ALL
        .into_iter()
        .all(|j| j == Joint::Pelvis || j.limits().contains(m.pose.angles(j)))
}

const FEET: [Joint; 2] = [Joint::LeftAnkle, Joint::RightAnkle];

#[test]
fn hips_dragged_down_over_held_feet_make_a_crouch() {
    let mut m = facing();
    let feet = FEET.map(|j| m.posed().at(j));
    let hips = mark(&m, Joint::Pelvis);
    let grab = m.grab(Joint::Pelvis, placement(), hips, &FEET);
    assert_eq!(grab.joint(), Joint::Pelvis);
    // The same pointer asked for twice moves the body once.
    for _ in 0..2 {
        m.drag(&grab, [hips[0], hips[1] + 15.0]);
    }
    assert!(close(m.pose.lift().y, -0.15, 1e-9), "{:?}", m.pose.lift());
    for (joint, was) in FEET.into_iter().zip(feet) {
        let now = m.posed().at(joint);
        assert!((now - was).length() < 1e-3, "{joint:?} moved to {now:?}");
    }
    assert!(m.pose.angles(Joint::LeftKnee).bend > 30.0);
    assert!(within_limits(&m));
}

#[test]
fn hips_dragged_with_nothing_held_take_the_feet_along() {
    let mut m = facing();
    let foot = m.posed().at(Joint::LeftAnkle);
    let hips = mark(&m, Joint::Pelvis);
    let grab = m.grab(Joint::Pelvis, placement(), hips, &[]);
    m.drag(&grab, [hips[0] + 10.0, hips[1] - 20.0]);
    let now = m.posed().at(Joint::LeftAnkle);
    assert!(close(now.y - foot.y, 0.2, 1e-9) && close(now.x - foot.x, 0.1, 1e-9));
    assert_eq!(m.pose.angles(Joint::LeftKnee).bend, 0.0);
}

#[test]
fn an_upper_arm_swings_so_the_elbow_stays_under_the_pointer() {
    let mut m = facing();
    let (shoulder, elbow) = (mark(&m, Joint::LeftShoulder), mark(&m, Joint::LeftElbow));
    let length = elbow[1] - shoulder[1];
    let grab = m.grab(Joint::LeftShoulder, placement(), elbow, &FEET);
    let to = [shoulder[0] + length * 0.8, shoulder[1] - length * 0.3];
    m.drag(&grab, to);
    let now = mark(&m, Joint::LeftElbow);
    assert!((now[0] - to[0]).hypot(now[1] - to[1]) < 0.5, "{now:?}");
    // Past the arm's length it lies flat in the picture, pointing at the pointer.
    m.drag(&grab, [shoulder[0] + 500.0, shoulder[1]]);
    let now = mark(&m, Joint::LeftElbow);
    assert!(close(now[0] - shoulder[0], length, 0.5) && close(now[1], shoulder[1], 0.5));
    assert!(within_limits(&m));
}

#[test]
fn a_forearm_or_a_shin_pulls_the_hand_or_foot() {
    let mut m = facing();
    let wrist = mark(&m, Joint::RightWrist);
    let elbow = mark(&m, Joint::RightElbow);
    let press = [(wrist[0] + elbow[0]) / 2.0, (wrist[1] + elbow[1]) / 2.0];
    let grab = m.grab(Joint::RightElbow, placement(), press, &[]);
    m.drag(&grab, [press[0] - 8.0, press[1] - 12.0]);
    let now = mark(&m, Joint::RightWrist);
    assert!(close(now[0], wrist[0] - 8.0, 0.5) && close(now[1], wrist[1] - 12.0, 0.5));
    assert!(m.pose.angles(Joint::RightElbow).bend < -5.0);
}

#[test]
fn a_held_foot_lets_go_for_its_own_leg_and_a_held_hand_stays_when_the_chest_moves() {
    let mut m = facing();
    let knee = mark(&m, Joint::LeftKnee);
    let grab = m.grab(Joint::LeftHip, placement(), knee, &FEET);
    m.drag(&grab, [knee[0] + 12.0, knee[1] - 4.0]);
    assert!(
        m.pose.angles(Joint::LeftHip).side > 10.0,
        "the leg swung out"
    );

    let mut m = facing();
    assert!(m.reach_point(Joint::LeftWrist, Vec3::new(0.2, 0.6, 0.15)));
    let hand = m.posed().at(Joint::LeftWrist);
    let neck = mark(&m, Joint::Neck);
    let grab = m.grab(Joint::Chest, placement(), neck, &[Joint::LeftWrist]);
    m.drag(&grab, [neck[0] + 4.0, neck[1] + 1.0]);
    assert!(
        m.pose.angles(Joint::Spine).side.abs() > 1.0,
        "the body leant"
    );
    assert!(
        m.pose.angles(Joint::Chest).side.abs() > 1.0,
        "at the chest too"
    );
    assert!((m.posed().at(Joint::LeftWrist) - hand).length() < 1e-3);
    assert!(within_limits(&m));
}

#[test]
fn the_head_follows_and_stops_where_a_neck_does() {
    let mut m = facing();
    let head = mark(&m, Joint::Head);
    let grab = m.grab(Joint::Head, placement(), [head[0], head[1] - 8.0], &[]);
    m.drag(&grab, [head[0] + 4.0, head[1] - 7.0]);
    let tilt = m.pose.angles(Joint::Neck).side.abs() + m.pose.angles(Joint::Head).side.abs();
    assert!(tilt > 5.0, "{tilt}");
    m.drag(&grab, [head[0] + 400.0, head[1] + 300.0]);
    assert!(within_limits(&m));
    let pose = m.pose;
    m.drag(&grab, [f64::NAN, 0.0]);
    assert_eq!(m.pose, pose, "a pointer that is nowhere moves nothing");
}

#[test]
fn a_hand_reaches_a_point_of_the_figures_space() {
    let mut m = facing();
    let goal = Vec3::new(0.1, 0.75, 0.2);
    assert!(m.reach_point(Joint::RightWrist, goal));
    assert!((m.posed().at(Joint::RightWrist) - goal).length() < 1e-3);
    assert!(!m.reach_point(Joint::RightWrist, Vec3::new(0.0, 3.0, 0.0)));
    assert!(!m.reach_point(Joint::RightElbow, goal));
    assert!(!m.reach_point(Joint::RightWrist, Vec3::new(f64::NAN, 0.0, 0.0)));
    assert!(within_limits(&m));
}

#[test]
fn a_request_pulls_hands_and_feet_and_refuses_what_cannot_be_reached() {
    let ask = |name: &str, point: [f64; 3]| {
        Request {
            reach: vec![(name.to_owned(), point)],
            ..Request::default()
        }
        .build()
    };
    let m = ask("left_wrist", [0.2, 0.7, 0.15]).expect("reached");
    let at = m.posed().at(Joint::LeftWrist);
    assert!((at - Vec3::new(0.2, 0.7, 0.15)).length() < 1e-3);
    assert_eq!(
        ask("left_elbow", [0.2, 0.7, 0.15]),
        Err(WireError::NotAnEnd("left_elbow".into()))
    );
    assert!(matches!(
        ask("left_wrist", [0.0, 3.0, 0.0]),
        Err(WireError::OutOfReach {
            joint: "left_wrist",
            ..
        })
    ));
    assert!(matches!(
        ask("left_wrist", [0.0, f64::INFINITY, 0.0]),
        Err(WireError::OutOfReach { .. })
    ));
    assert_eq!(
        ask("tail", [0.0; 3]),
        Err(WireError::UnknownJoint("tail".into()))
    );
    let ends = crate::catalog()["reach"].as_array().expect("ends").len();
    assert_eq!(ends, 4);
}

#[test]
fn the_floor_is_a_grid_where_the_figure_first_stood() {
    let mut m = facing();
    m.view = crate::View::new(20.0, 30.0, 30.0);
    let floor = m.floor(placement());
    assert_eq!(floor.len(), 14);
    assert!(floor.iter().flatten().flatten().all(|v| v.is_finite()));
    // Lifting the figure leaves the floor where it is: the camera stays on the ground.
    let hips = m.marks(placement())[Joint::Pelvis.index()].at[1];
    m.pose.set_lift(Vec3::new(0.0, 0.3, 0.0));
    assert_eq!(m.floor(placement()), floor);
    assert!(m.marks(placement())[Joint::Pelvis.index()].at[1] < hips - 10.0);
}
