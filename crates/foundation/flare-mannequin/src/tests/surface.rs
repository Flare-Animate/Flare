use super::front;
use crate::kinematics::pose_figure;
use crate::skin::skin;
use crate::{Angles, Body, Joint, Look, Mannequin, Placement, Pose, Vec3};

const FLAT: Look = Look {
    picked: None,
    guides: false,
    shadow: false,
};

/// The top of the figure seen flat from the front at each `x` across it, in figure units above
/// the neck joint: the first opaque pixel down each column of a large picture.
fn tops(m: &Mannequin, xs: &[f64]) -> Vec<f64> {
    let m = Mannequin {
        view: front(),
        ..*m
    };
    let placement = Placement::new([300.0, 1000.0], 1000.0).expect("placement");
    let picture = m.render(placement, 600, 1100, FLAT).expect("picture");
    let projector = m.projector(placement);
    let neck = m.posed().at(Joint::Neck);
    let at = |p: Vec3| projector.project(p).at;
    let per_unit = at(neck)[1] - at(neck + Vec3::new(0.0, 1.0, 0.0))[1];
    let width = picture.width() as usize;
    xs.iter()
        .map(|&x| {
            let [col, neck_row] = at(Vec3::new(x, neck.y, 0.0));
            let col = col.round() as usize;
            let row = (0..picture.height() as usize)
                .find(|&row| picture.rgba()[(row * width + col) * 4 + 3] > 128)
                .expect("the figure is in the column");
            (neck_row - row as f64) / per_unit
        })
        .collect()
}

/// The shares of the chest and a shoulder each point of the surface is moved by, where it has
/// both, with where it rests.
fn shared_with_the_chest(m: &Mannequin, shoulder: Joint) -> Vec<(usize, Vec3)> {
    let (mesh, _) = m.surface();
    mesh.vertices
        .iter()
        .enumerate()
        .filter(|(_, v)| {
            let [(a, sa), (b, sb)] = v.bones;
            a == Joint::Chest && b == shoulder && sa > 0.0 && sb > 0.0
        })
        .map(|(i, v)| (i, v.rest))
        .collect()
}

#[test]
fn the_shoulder_line_falls_from_the_neck_over_the_deltoid() {
    for body in Body::ALL {
        let m = Mannequin::new(body);
        let x = m.posed().at(Joint::LeftShoulder).x;
        // From beside the neck to past the joint, where the deltoid caps it.
        let xs: Vec<f64> = (0..9).map(|i| x * (0.55 + 0.09 * f64::from(i))).collect();
        let tops = tops(&m, &xs);
        for (i, pair) in tops.windows(2).enumerate() {
            // The deltoid may crown the joint a little, as a shoulder does; a step up is an arm
            // set beside a square torso instead of a shoulder.
            assert!(
                pair[1] - pair[0] < 0.003,
                "{body:?}: the line steps up {:.4} at {:.3}",
                pair[1] - pair[0],
                xs[i + 1]
            );
        }
        assert!(tops[8] < tops[0] - 0.02, "{body:?}: the shoulders slope");
        assert!(
            tops[7] > -0.03,
            "{body:?}: the deltoid caps the joint, past it"
        );
    }
}

#[test]
fn a_raised_arm_turns_the_shoulder_round_the_joint_instead_of_folding_it() {
    let mut m = Mannequin::default();
    m.pose
        .set(Joint::LeftShoulder, Angles::new(-30.0, 0.0, 120.0));
    let build = m.build();
    let rest = pose_figure(&build, &Pose::default());
    let posed = m.posed();
    let (_, skinned) = m.surface();
    let shared = shared_with_the_chest(&m, Joint::LeftShoulder);
    assert!(
        shared.len() > 20,
        "the deltoid's top is shared with the chest"
    );
    for (i, at_rest) in shared {
        let before = (at_rest - rest.at(Joint::LeftShoulder)).length();
        let after = (skinned.points[i] - posed.at(Joint::LeftShoulder)).length();
        // Points set between where each joint would take them would come closer.
        assert!(
            (before - after).abs() < 1e-9,
            "a point {before:.4} from the joint ends {after:.4} from it"
        );
    }
}

#[test]
fn a_twisted_arm_turns_its_shoulder_cap_whole() {
    let mut twisted = Mannequin::default();
    twisted
        .pose
        .set(Joint::RightShoulder, Angles::new(0.0, 80.0, 0.0));
    let (mesh, skinned) = twisted.surface();
    let rest = pose_figure(&twisted.build(), &Pose::default());
    let posed = twisted.posed();
    let shoulder = rest.at(Joint::RightShoulder);
    for (i, at_rest) in shared_with_the_chest(&twisted, Joint::RightShoulder) {
        // All of the twist, about the bone, with nothing left behind with the chest.
        let whole = posed.at(Joint::RightShoulder)
            + posed.frame(Joint::RightShoulder).apply(at_rest - shoulder);
        assert!(
            (skinned.points[i] - whole).length() < 1e-9,
            "point {i} of {} is left behind",
            mesh.vertices.len()
        );
    }
    let still = skin(&mesh, &rest, &rest);
    assert!(
        still
            .points
            .iter()
            .zip(&mesh.vertices)
            .all(|(p, v)| (*p - v.rest).length() < 1e-12),
        "at rest the surface stays where it was built"
    );
}
