use proptest::prelude::*;

use super::{close, front, placement};
use crate::{Angles, Body, Joint, Look, Mannequin, Placement, View};

#[test]
fn a_flat_front_view_puts_the_figure_its_height_round_the_middle() {
    let m = Mannequin {
        view: front(),
        ..Mannequin::default()
    };
    let (lo, hi) = m.bounds(placement(), false);
    // The middle (0.5 up) is at y = 500; the soles at 0 are 50 below, the crown 50 above.
    assert!(close(lo[1], 450.0, 1.5), "crown at {}", lo[1]);
    assert!(close(hi[1], 550.0, 1.5), "soles at {}", hi[1]);
    assert!(close(lo[0] + hi[0], 1000.0, 1e-6), "level about the middle");
    let (_, with_shadow) = m.bounds(placement(), true);
    assert!(with_shadow[1] >= hi[1]);
}

#[test]
fn a_figure_seen_from_the_side_is_narrower_and_turns_its_left_toward_the_right_of_the_picture() {
    let mut m = Mannequin {
        view: front(),
        ..Mannequin::default()
    };
    m.pose = crate::preset("t_pose").expect("t pose");
    let width = |m: &Mannequin| {
        let (lo, hi) = m.bounds(placement(), false);
        hi[0] - lo[0]
    };
    let facing = width(&m);
    let left = m.marks(placement())[Joint::LeftShoulder.index()].at[0];
    assert!(
        left > 500.0,
        "facing the viewer, the figure's left is on the picture's right"
    );
    m.view = View::new(90.0, 0.0, 0.0);
    assert!(width(&m) < facing * 0.4, "{} of {facing}", width(&m));
}

#[test]
fn perspective_makes_the_nearer_end_larger() {
    let mut m = Mannequin {
        view: View::new(0.0, 0.0, 80.0),
        ..Mannequin::default()
    };
    m.pose
        .set(Joint::LeftShoulder, Angles::new(-90.0, 0.0, 0.0));
    let flat = {
        let mut f = m;
        f.view = front();
        f
    };
    let reach = |m: &Mannequin| {
        let marks = m.marks(placement());
        let (a, b) = (
            marks[Joint::LeftShoulder.index()].at,
            marks[Joint::LeftWrist.index()].at,
        );
        (a[0] - b[0]).hypot(a[1] - b[1])
    };
    assert!(
        reach(&flat) < 2.0,
        "an arm pointing at the viewer is seen end on"
    );
    let marks = m.marks(placement());
    assert!(marks[Joint::LeftWrist.index()].depth > marks[Joint::LeftShoulder.index()].depth);
    let (lo, hi) = m.bounds(placement(), false);
    let (flo, fhi) = flat.bounds(placement(), false);
    assert!(
        hi[0] - lo[0] > fhi[0] - flo[0],
        "the near hand looms larger"
    );
}

#[test]
fn a_placement_must_be_finite_and_positive() {
    assert!(Placement::new([0.0, 0.0], 0.0).is_none());
    assert!(Placement::new([f64::NAN, 0.0], 10.0).is_none());
    assert!(Placement::new([0.0, 0.0], 1e9).is_none());
    assert!(Placement::new([0.0, 0.0], 1.0).is_some());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]
    #[test]
    fn any_pose_from_any_side_makes_a_finite_picture(
        angles in proptest::collection::vec((-200.0f64..200.0, -200.0f64..200.0, -200.0f64..200.0), 17),
        yaw in -400.0f64..400.0,
        pitch in -120.0f64..120.0,
        fov in -10.0f64..120.0,
        female in any::<bool>(),
    ) {
        let mut m = Mannequin::new(if female { Body::Female } else { Body::Male });
        for (joint, (b, t, s)) in Joint::ALL.into_iter().zip(angles) {
            m.pose.set(joint, Angles::new(b, t, s));
        }
        m.view = View::new(yaw, pitch, fov);
        let (lo, hi) = m.bounds(placement(), true);
        prop_assert!(lo.iter().chain(&hi).all(|v| v.is_finite()));
        let small = Placement::new([40.0, 40.0], 60.0).expect("placement");
        let picture = m.render(small, 80, 80, Look::default()).expect("picture");
        prop_assert_eq!(picture.rgba().len(), 80 * 80 * 4);
        for joint in Joint::ALL {
            prop_assert!(joint == Joint::Pelvis || joint.limits().contains(m.pose.angles(joint)));
        }
    }
}
