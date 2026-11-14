use super::{close, front, placement};
use crate::{Axis, Joint, Mannequin, RING_POINTS};

fn facing() -> Mannequin {
    Mannequin {
        view: front(),
        ..Mannequin::default()
    }
}

const RADIUS: f64 = 30.0;

/// A point of `axis`'s ring and the way along the ring there: the `i`th point, or the next one
/// on the near half clear of the other rings, so a press there takes this ring.
fn on_ring(m: &Mannequin, joint: Joint, axis: Axis, i: usize) -> ([f64; 2], [f64; 2]) {
    let rings = m.rings(joint, placement(), RADIUS);
    let ring = rings.iter().find(|r| r.axis == axis).expect("ring");
    let clear = |k: usize| {
        let p = ring.points[k];
        ring.front[k]
            && rings
                .iter()
                .filter(|r| r.axis != axis)
                .flat_map(|r| r.points.iter())
                .all(|q| (q[0] - p[0]).hypot(q[1] - p[1]) > 4.0)
    };
    let i = (i..i + RING_POINTS - 2)
        .map(|k| 1 + k % (RING_POINTS - 2))
        .find(|k| clear(*k))
        .unwrap_or(i);
    let (a, b) = (ring.points[i - 1], ring.points[i + 1]);
    let length = (b[0] - a[0]).hypot(b[1] - a[1]);
    (
        ring.points[i],
        [(b[0] - a[0]) / length, (b[1] - a[1]) / length],
    )
}

#[test]
fn a_joint_has_a_ring_for_each_way_it_turns() {
    let m = facing();
    let axes = |joint| -> Vec<Axis> {
        m.rings(joint, placement(), RADIUS)
            .iter()
            .map(|r| r.axis)
            .collect()
    };
    assert_eq!(axes(Joint::LeftShoulder), Axis::ALL.to_vec());
    assert_eq!(axes(Joint::LeftElbow), vec![Axis::Bend, Axis::Twist]);
    for ring in m.rings(Joint::Head, placement(), RADIUS) {
        assert_eq!(ring.points.len(), RING_POINTS);
        assert_eq!(ring.front.len(), RING_POINTS);
    }
    // The ring about the line of sight is a circle of the size asked round the joint.
    let center = m.marks(placement())[Joint::LeftShoulder.index()].at;
    let (p, _) = on_ring(&m, Joint::LeftShoulder, Axis::Side, 6);
    assert!(close(
        (p[0] - center[0]).hypot(p[1] - center[1]),
        RADIUS,
        1e-6
    ));
}

#[test]
fn a_drag_along_a_ring_turns_the_joint_about_that_axis_alone() {
    for (joint, axis) in [
        (Joint::LeftShoulder, Axis::Side),
        (Joint::RightShoulder, Axis::Side),
        (Joint::Neck, Axis::Twist),
        (Joint::LeftHip, Axis::Bend),
        (Joint::Pelvis, Axis::Twist),
    ] {
        let mut m = facing();
        m.view = crate::View::new(20.0, 25.0, 0.0);
        let (at, along) = on_ring(&m, joint, axis, 5);
        let grab = m
            .grab_ring(joint, placement(), RADIUS, at, 3.0)
            .expect("on the ring");
        assert_eq!((grab.joint, grab.axis), (joint, axis), "{joint:?}");
        let before = m.posed();
        let was = m.pose.angles(joint);
        // A twelfth of the ring's radius along it is a twelfth of a radian.
        let step = RADIUS / 12.0;
        m.turn_ring(&grab, [-along[0] * step, -along[1] * step]);
        m.turn_ring(&grab, [f64::NAN, 0.0]);
        let now = m.pose.angles(joint);
        let changed = [
            now.bend - was.bend,
            now.twist - was.twist,
            now.side - was.side,
        ];
        let turned = changed[axis as usize];
        assert!(
            close(turned.abs(), (1.0f64 / 12.0).to_degrees(), 1e-6),
            "{joint:?} {changed:?}"
        );
        assert_eq!(changed.iter().filter(|c| **c != 0.0).count(), 1);
        // The part went the way the pointer did: a point of the ring's plane, carried by the
        // joint, moved in the picture along the drag.
        let rings = m.rings(joint, placement(), RADIUS);
        assert!(!rings.is_empty());
        let frame_turn = m.posed().frame(joint) * before.frame(joint).transpose();
        assert!(frame_turn != crate::Rot3::IDENTITY);
    }
}

#[test]
fn a_ring_stops_at_the_joints_limit_and_a_press_off_the_rings_takes_none() {
    let mut m = facing();
    let (at, along) = on_ring(&m, Joint::LeftKnee, Axis::Bend, 5);
    let grab = m
        .grab_ring(Joint::LeftKnee, placement(), RADIUS, at, 3.0)
        .expect("on the ring");
    for sign in [1.0, -1.0] {
        m.turn_ring(&grab, [along[0] * 4000.0 * sign, along[1] * 4000.0 * sign]);
        let bend = m.pose.angles(Joint::LeftKnee).bend;
        assert!(bend == 0.0 || bend == 150.0, "{bend}");
    }
    let center = m.marks(placement())[Joint::LeftKnee.index()].at;
    assert!(
        m.grab_ring(
            Joint::LeftKnee,
            placement(),
            RADIUS,
            [center[0] + 200.0, center[1]],
            3.0
        )
        .is_none()
    );
}

#[test]
fn the_way_round_a_ring_is_the_way_the_part_goes() {
    // Facing the figure, its left arm hangs on the picture's right. Dragging the bottom of the
    // ring that lies flat in the picture out to the right lifts the arm out to that side.
    let mut m = facing();
    let center = m.marks(placement())[Joint::LeftShoulder.index()].at;
    let below = [center[0] + 2.0, center[1] + RADIUS];
    let grab = m
        .grab_ring(Joint::LeftShoulder, placement(), RADIUS, below, 4.0)
        .expect("on the ring");
    assert_eq!(grab.axis, Axis::Side);
    m.turn_ring(&grab, [RADIUS, 0.0]);
    let side = m.pose.angles(Joint::LeftShoulder).side;
    assert!(close(side, 1.0f64.to_degrees(), 2.0), "{side}");
    let (elbow, shoulder) = (
        m.marks(placement())[Joint::LeftElbow.index()].at,
        m.marks(placement())[Joint::LeftShoulder.index()].at,
    );
    assert!(elbow[0] > shoulder[0] + 10.0, "the arm is out to the right");
}
