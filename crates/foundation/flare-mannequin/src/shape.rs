//! The shape of the body: cross-sections of the torso, neck, head, arms, legs, hands and feet,
//! for a man's build and a woman's, laid along the skeleton at rest and scaled by the
//! proportions. The numbers are in figure units (the default figure one unit tall), drawn from
//! the usual eight-heads figure of life drawing.

use crate::body::Build;
use crate::kinematics::Posed;
use crate::math::Vec3;
use crate::skeleton::Joint;
use crate::skin::{Bones, Mesh, Ring, Tube, blend, only};

/// The spacing of the guide lines along the body.
const GRID: f64 = 0.06;
const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);

/// A torso cross-section: the joint it is measured from, the height above it, the half width,
/// the depth in front and behind, and how far forward its middle is.
type TorsoRow = (Joint, f64, f64, f64, f64, f64);

const MALE_TORSO: [TorsoRow; 17] = [
    (Joint::Pelvis, -0.078, 0.0, 0.0, 0.0, 0.0),
    (Joint::Pelvis, -0.072, 0.04, 0.03, 0.04, 0.0),
    (Joint::Pelvis, -0.055, 0.084, 0.055, 0.065, 0.0),
    (Joint::Pelvis, -0.03, 0.103, 0.06, 0.072, 0.0),
    (Joint::Pelvis, 0.0, 0.1, 0.06, 0.072, 0.0),
    (Joint::Pelvis, 0.03, 0.091, 0.058, 0.062, 0.0),
    (Joint::Spine, 0.02, 0.082, 0.056, 0.052, 0.0),
    (Joint::Spine, 0.05, 0.08, 0.056, 0.05, 0.0),
    (Joint::Chest, -0.025, 0.085, 0.06, 0.054, 0.0),
    (Joint::Chest, 0.01, 0.092, 0.065, 0.058, 0.0),
    (Joint::Chest, 0.045, 0.097, 0.07, 0.062, 0.0),
    (Joint::Chest, 0.075, 0.1, 0.066, 0.062, 0.0),
    (Joint::Chest, 0.098, 0.1, 0.052, 0.058, -0.004),
    (Joint::Neck, -0.012, 0.07, 0.04, 0.048, -0.006),
    (Joint::Neck, 0.005, 0.045, 0.034, 0.04, -0.006),
    (Joint::Neck, 0.02, 0.03, 0.028, 0.032, -0.006),
    (Joint::Neck, 0.03, 0.0, 0.0, 0.0, -0.006),
];

const FEMALE_TORSO: [TorsoRow; 17] = [
    (Joint::Pelvis, -0.078, 0.0, 0.0, 0.0, 0.0),
    (Joint::Pelvis, -0.072, 0.042, 0.03, 0.045, 0.0),
    (Joint::Pelvis, -0.055, 0.092, 0.055, 0.072, 0.0),
    (Joint::Pelvis, -0.03, 0.11, 0.058, 0.08, 0.0),
    (Joint::Pelvis, 0.0, 0.106, 0.056, 0.078, 0.0),
    (Joint::Pelvis, 0.03, 0.092, 0.054, 0.064, 0.0),
    (Joint::Spine, 0.02, 0.074, 0.05, 0.05, 0.0),
    (Joint::Spine, 0.05, 0.068, 0.05, 0.047, 0.0),
    (Joint::Chest, -0.025, 0.074, 0.052, 0.049, 0.0),
    (Joint::Chest, 0.01, 0.08, 0.056, 0.052, 0.0),
    (Joint::Chest, 0.045, 0.084, 0.058, 0.054, 0.0),
    (Joint::Chest, 0.075, 0.086, 0.055, 0.054, 0.0),
    (Joint::Chest, 0.098, 0.086, 0.046, 0.05, -0.004),
    (Joint::Neck, -0.012, 0.06, 0.035, 0.042, -0.006),
    (Joint::Neck, 0.005, 0.04, 0.03, 0.035, -0.006),
    (Joint::Neck, 0.02, 0.026, 0.025, 0.028, -0.006),
    (Joint::Neck, 0.03, 0.0, 0.0, 0.0, -0.006),
];

/// A head cross-section, measured from the head joint and scaled by the head's size: height,
/// half width, depth in front and behind, and how far forward its middle is.
const HEAD: [(f64, f64, f64, f64, f64); 11] = [
    (-0.008, 0.0, 0.0, 0.0, 0.028),
    (-0.004, 0.012, 0.01, 0.01, 0.028),
    (0.006, 0.026, 0.026, 0.018, 0.018),
    (0.02, 0.036, 0.044, 0.032, 0.006),
    (0.04, 0.043, 0.05, 0.048, 0.0),
    (0.06, 0.046, 0.052, 0.058, 0.0),
    (0.08, 0.047, 0.05, 0.062, -0.002),
    (0.098, 0.043, 0.04, 0.056, -0.004),
    (0.11, 0.031, 0.027, 0.041, -0.006),
    (0.117, 0.014, 0.012, 0.017, -0.006),
    (0.12, 0.0, 0.0, 0.0, -0.006),
];
/// Where the eyes are on the head, which a guide line marks.
const EYE_LINE: f64 = 0.062;

/// A limb cross-section: how far down the limb (in the default figure's lengths), the reach
/// outward and inward, in front and behind, and the middle's step outward and forward.
type LimbRow = (f64, f64, f64, f64, f64, f64, f64);

const ARM: [LimbRow; 13] = [
    (-0.016, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
    (-0.012, 0.022, 0.018, 0.026, 0.026, 0.002, 0.0),
    (0.0, 0.034, 0.028, 0.036, 0.036, 0.004, 0.0),
    (0.02, 0.037, 0.029, 0.036, 0.035, 0.005, 0.0),
    (0.05, 0.034, 0.027, 0.033, 0.031, 0.003, 0.0),
    (0.09, 0.03, 0.026, 0.031, 0.028, 0.0, 0.002),
    (0.13, 0.026, 0.025, 0.026, 0.026, 0.0, 0.0),
    (0.17, 0.024, 0.023, 0.023, 0.024, 0.0, -0.002),
    (0.2, 0.028, 0.025, 0.026, 0.025, 0.0, 0.0),
    (0.25, 0.025, 0.022, 0.022, 0.021, 0.0, 0.0),
    (0.3, 0.019, 0.017, 0.015, 0.014, 0.0, 0.0),
    (0.322, 0.018, 0.016, 0.013, 0.012, 0.0, 0.0),
    (0.335, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0),
];
/// Where the elbow is along the arm's rows.
const ELBOW_AT: f64 = 0.17;

const LEG: [LimbRow; 14] = [
    (-0.06, 0.0, 0.0, 0.0, 0.0, 0.0, -0.005),
    (-0.05, 0.035, 0.03, 0.04, 0.045, 0.0, -0.005),
    (-0.025, 0.05, 0.048, 0.054, 0.058, 0.0, -0.004),
    (0.0, 0.053, 0.05, 0.054, 0.056, 0.0, 0.0),
    (0.05, 0.05, 0.046, 0.05, 0.049, 0.0, 0.004),
    (0.12, 0.043, 0.04, 0.044, 0.041, 0.0, 0.004),
    (0.19, 0.036, 0.034, 0.036, 0.034, 0.0, 0.002),
    (0.235, 0.033, 0.031, 0.034, 0.031, 0.0, 0.003),
    (0.27, 0.034, 0.032, 0.032, 0.037, 0.0, -0.004),
    (0.31, 0.034, 0.031, 0.029, 0.04, 0.0, -0.008),
    (0.36, 0.028, 0.026, 0.025, 0.031, 0.0, -0.006),
    (0.41, 0.021, 0.02, 0.02, 0.023, 0.0, -0.004),
    (0.45, 0.02, 0.019, 0.019, 0.02, 0.0, -0.004),
    (0.465, 0.0, 0.0, 0.0, 0.0, 0.0, -0.004),
];
/// Where the knee is along the leg's rows.
const KNEE_AT: f64 = 0.235;

/// The palm, down from the wrist: the reach to the back of the hand and to the palm, toward
/// the thumb and away from it.
const PALM: [(f64, f64, f64, f64, f64); 6] = [
    (-0.004, 0.0, 0.0, 0.0, 0.0),
    (0.0, 0.013, 0.013, 0.018, 0.018),
    (0.02, 0.014, 0.014, 0.025, 0.025),
    (0.045, 0.012, 0.012, 0.028, 0.027),
    (0.058, 0.01, 0.01, 0.026, 0.025),
    (0.064, 0.0, 0.0, 0.0, 0.0),
];
/// The fingers, thumb side first: how far toward the thumb each starts, its length and
/// thickness.
const FINGERS: [(f64, f64, f64); 4] = [
    (0.019, 0.044, 0.0062),
    (0.0065, 0.048, 0.0064),
    (-0.0065, 0.045, 0.0062),
    (-0.019, 0.036, 0.0055),
];

/// The foot, forward from the heel: the reach outward and inward, up and down, and how far
/// below the ankle its middle is.
const FOOT: [(f64, f64, f64, f64, f64, f64); 10] = [
    (-0.044, 0.0, 0.0, 0.0, 0.0, -0.02),
    (-0.04, 0.014, 0.014, 0.012, 0.014, -0.022),
    (-0.03, 0.021, 0.021, 0.018, 0.018, -0.022),
    (-0.012, 0.023, 0.023, 0.026, 0.022, -0.018),
    (0.01, 0.023, 0.024, 0.03, 0.026, -0.014),
    (0.035, 0.027, 0.025, 0.022, 0.02, -0.02),
    (0.065, 0.031, 0.027, 0.016, 0.015, -0.025),
    (0.095, 0.032, 0.028, 0.012, 0.012, -0.028),
    (0.118, 0.028, 0.025, 0.01, 0.01, -0.03),
    (0.13, 0.0, 0.0, 0.0, 0.0, -0.032),
];

/// A woman's breast, forward from inside the chest: the reach outward and inward, up and down,
/// and how far its middle drops.
const BREAST: [(f64, f64, f64, f64, f64, f64); 6] = [
    (0.03, 0.0, 0.0, 0.0, 0.0, 0.0),
    (0.035, 0.042, 0.036, 0.036, 0.044, 0.0),
    (0.05, 0.04, 0.034, 0.032, 0.042, -0.004),
    (0.068, 0.032, 0.028, 0.024, 0.034, -0.01),
    (0.08, 0.02, 0.017, 0.014, 0.02, -0.013),
    (0.086, 0.0, 0.0, 0.0, 0.0, -0.014),
];

fn tube(u: Vec3, w: Vec3, around: usize, lines: usize, rings_lined: bool) -> Tube {
    Tube {
        u,
        w,
        around,
        lines,
        rings_lined,
        squareness: 2.0,
    }
}

/// The joint whose turn moves a point of the torso at height `y`, shared near each joint.
fn torso_bones(rest: &Posed, y: f64) -> Bones {
    let (spine, chest, neck) = (
        rest.at(Joint::Spine).y,
        rest.at(Joint::Chest).y,
        rest.at(Joint::Neck).y,
    );
    if y < chest - 0.03 {
        blend(Joint::Pelvis, Joint::Spine, (y - spine + 0.03) / 0.06)
    } else if y < neck - 0.015 {
        blend(Joint::Spine, Joint::Chest, (y - chest + 0.03) / 0.06)
    } else {
        blend(Joint::Chest, Joint::Neck, (y - neck + 0.015) / 0.03)
    }
}

/// The surface of the whole body at rest.
pub(crate) fn body(b: &Build, rest: &Posed) -> Mesh {
    let mut mesh = Mesh::default();
    torso(&mut mesh, b, rest);
    head(&mut mesh, b, rest);
    for side in [1.0, -1.0] {
        arm(&mut mesh, b, rest, side);
        leg(&mut mesh, b, rest, side);
        if b.female {
            breast(&mut mesh, b, rest, side);
        }
    }
    mesh
}

fn torso(mesh: &mut Mesh, b: &Build, rest: &Posed) {
    let rows = if b.female { &FEMALE_TORSO } else { &MALE_TORSO };
    let bottom = rest.at(Joint::Pelvis).y + rows[0].1 * b.torso;
    let rings: Vec<Ring> = rows
        .iter()
        .map(|&(joint, dy, x, front, back, forward)| {
            let y = rest.at(joint).y + dy * b.torso;
            let wide = match joint {
                Joint::Pelvis => b.hips,
                Joint::Spine => (b.hips * b.shoulders).sqrt(),
                _ => b.shoulders,
            };
            let x = x * wide * b.thick;
            Ring {
                center: Vec3::new(0.0, y, forward),
                u: (x, x),
                w: (front * b.thick, back * b.thick),
                bones: torso_bones(rest, y),
                along: (y - bottom) / GRID,
            }
        })
        .collect();
    let mut shape = tube(X, Z, 32, 8, true);
    shape.squareness = 2.3;
    mesh.tube(shape, &rings);
    // The neck, from inside the shoulders to inside the head.
    let (neck, top) = (rest.at(Joint::Neck), rest.at(Joint::Head));
    let r = if b.female { 0.027 } else { 0.031 } * b.thick;
    let rows = [
        (neck.y - 0.03, 0.0, -0.004),
        (neck.y - 0.025, r, -0.004),
        (neck.y + 0.01, r * 0.95, -0.002),
        (top.y, r * 0.92, 0.002),
        (top.y + 0.025, r * 0.92, 0.004),
        (top.y + 0.03, 0.0, 0.004),
    ];
    let rings: Vec<Ring> = rows
        .iter()
        .map(|&(y, r, z)| Ring {
            center: Vec3::new(0.0, y, z),
            u: (r, r),
            w: (r * 1.08, r),
            bones: blend(Joint::Neck, Joint::Head, (y - top.y + 0.01) / 0.03),
            along: 0.5,
        })
        .collect();
    mesh.tube(tube(X, Z, 16, 2, false), &rings);
}

fn head(mesh: &mut Mesh, b: &Build, rest: &Posed) {
    let at = rest.at(Joint::Head);
    let h = b.head;
    let rings: Vec<Ring> = HEAD
        .iter()
        .map(|&(dy, x, front, back, forward)| Ring {
            center: at + Vec3::new(0.0, dy * h, forward * h),
            u: (x * h, x * h),
            w: (front * h, back * h),
            bones: only(Joint::Head),
            along: (dy - EYE_LINE) / 0.04,
        })
        .collect();
    mesh.tube(tube(X, Z, 32, 4, true), &rings);
    // A nose, so the way the head faces reads from any side.
    let nose = |z: f64, r: f64, y: f64| Ring {
        center: at + Vec3::new(0.0, y * h, z * h),
        u: (r * h, r * h),
        w: (r * h * 1.2, r * h),
        bones: only(Joint::Head),
        along: 0.5,
    };
    let rings = [
        nose(0.04, 0.0, 0.05),
        nose(0.044, 0.006, 0.048),
        nose(0.055, 0.007, 0.036),
        nose(0.06, 0.005, 0.034),
        nose(0.062, 0.0, 0.033),
    ];
    mesh.tube(tube(X, Y, 10, 0, false), &rings);
}

/// Rings down a limb from `joint`'s place at rest: `rows` give the cross-sections in the
/// default figure's lengths, `place` turns such a length into how far down the limb it is
/// here, and `bones` which joints move it there.
#[allow(clippy::too_many_arguments)]
fn limb(
    mesh: &mut Mesh,
    rest: &Posed,
    joint: Joint,
    side: f64,
    rows: &[LimbRow],
    place: impl Fn(f64) -> f64,
    thick: impl Fn(f64) -> f64,
    bones: impl Fn(f64) -> Bones,
) {
    let at = rest.at(joint);
    let out = X.scale(side);
    let rings: Vec<Ring> = rows
        .iter()
        .map(|&(t, outward, inward, front, back, step, forward)| {
            let k = thick(t);
            Ring {
                center: at + out.scale(step * k) + Vec3::new(0.0, -place(t), forward * k),
                u: (outward * k, inward * k),
                w: (front * k, back * k),
                bones: bones(t),
                along: t / GRID,
            }
        })
        .collect();
    mesh.tube(tube(out, Z, 24, 4, true), &rings);
}

fn arm(mesh: &mut Mesh, b: &Build, rest: &Posed, side: f64) {
    use Joint::*;
    let (shoulder, elbow, wrist) = if side > 0.0 {
        (LeftShoulder, LeftElbow, LeftWrist)
    } else {
        (RightShoulder, RightElbow, RightWrist)
    };
    let (upper, fore) = (b.upper_arm, b.forearm);
    let place = |t: f64| {
        if t <= ELBOW_AT {
            t * upper / ELBOW_AT
        } else {
            upper + (t - ELBOW_AT) * fore / 0.15
        }
    };
    let (ku, kf) = (b.upper_arm_r.0 / 0.038, b.forearm_r.0 / 0.031);
    let thick = |t: f64| if t <= ELBOW_AT { ku } else { kf };
    let bones = |t: f64| blend(shoulder, elbow, (t - 0.14) / 0.06);
    limb(mesh, rest, shoulder, side, &ARM, place, thick, bones);
    hand(mesh, b, rest, wrist, side);
}

fn leg(mesh: &mut Mesh, b: &Build, rest: &Posed, side: f64) {
    use Joint::*;
    let (hip, knee, ankle) = if side > 0.0 {
        (LeftHip, LeftKnee, LeftAnkle)
    } else {
        (RightHip, RightKnee, RightAnkle)
    };
    let (thigh, shin) = (b.thigh, b.shin);
    let place = |t: f64| {
        if t <= KNEE_AT {
            t * thigh / KNEE_AT
        } else {
            thigh + (t - KNEE_AT) * shin / 0.215
        }
    };
    let (kt, ks) = (b.thigh_r.0 / 0.062, b.shin_r.0 / 0.042);
    let thick = |t: f64| if t <= KNEE_AT { kt } else { ks };
    // The top of the thigh, inside the hips, moves partly with them, so a raised leg does not
    // pull it out through the seat.
    let bones = |t: f64| {
        if t <= 0.0 {
            blend(Pelvis, hip, 0.35 + (t + 0.06) / 0.06 * 0.65)
        } else {
            blend(hip, knee, (t - 0.2) / 0.07)
        }
    };
    limb(mesh, rest, hip, side, &LEG, place, thick, bones);
    foot(mesh, b, rest, ankle, side);
}

fn hand(mesh: &mut Mesh, b: &Build, rest: &Posed, wrist: Joint, side: f64) {
    let at = rest.at(wrist);
    let out = X.scale(side);
    let (long, k) = (b.hand / 0.1, b.limb);
    // In the hand's own terms: outward, down from the wrist, toward the thumb (forward).
    let point = |o: f64, t: f64, f: f64| at + out.scale(o) + Vec3::new(0.0, -t * long, f * k);
    let rings: Vec<Ring> = PALM
        .iter()
        .map(|&(t, back, palm, thumb, away)| Ring {
            center: point(0.0, t, 0.0),
            u: (back * k, palm * k),
            w: (thumb * k, away * k),
            bones: only(wrist),
            along: 0.5,
        })
        .collect();
    mesh.tube(tube(out, Z, 16, 0, false), &rings);
    // Each finger runs straight, then curls a little toward the palm.
    let (curl_s, curl_c) = 25f64.to_radians().sin_cos();
    for (f, length, r) in FINGERS {
        let start = (0.0, 0.05, f);
        let knuckle = (0.0, 0.05 + length * 0.55, f);
        let rest_of = length * 0.45;
        let end = (-curl_s * rest_of, knuckle.1 + curl_c * rest_of, f);
        finger(mesh, &point, out, wrist, [start, knuckle, end], r * k);
    }
    let thumb = [
        (-0.006, 0.012, 0.018),
        (-0.012, 0.04, 0.034),
        (-0.016, 0.06, 0.036),
    ];
    finger(mesh, &point, out, wrist, thumb, 0.0075 * k);
}

fn finger(
    mesh: &mut Mesh,
    point: &impl Fn(f64, f64, f64) -> Vec3,
    out: Vec3,
    wrist: Joint,
    path: [(f64, f64, f64); 3],
    r: f64,
) {
    let at = |(o, t, f): (f64, f64, f64)| point(o, t, f);
    let [a, m, e] = path.map(at);
    let toward = |p: Vec3, q: Vec3, by: f64| p + (q - p).normalized().unwrap_or(Y).scale(by);
    let ring = |center: Vec3, r: f64| Ring {
        center,
        u: (r, r),
        w: (r, r),
        bones: only(wrist),
        along: 0.5,
    };
    let rings = [
        ring(a, 0.0),
        ring(toward(a, m, r * 0.5), r),
        ring(m, r * 0.95),
        ring(toward(e, m, r * 0.6), r * 0.85),
        ring(e, 0.0),
    ];
    mesh.tube(tube(out, Z, 8, 0, false), &rings);
}

fn foot(mesh: &mut Mesh, b: &Build, rest: &Posed, ankle: Joint, side: f64) {
    let at = rest.at(ankle);
    let out = X.scale(side);
    let (long, k) = (b.limb.sqrt(), b.limb);
    let rings: Vec<Ring> = FOOT
        .iter()
        .map(|&(z, outward, inward, up, down, drop)| Ring {
            center: at + Vec3::new(0.0, drop, z * long),
            u: (outward * k, inward * k),
            // The sole stays on the ground whatever the build.
            w: (up, down),
            bones: only(ankle),
            along: 0.5,
        })
        .collect();
    mesh.tube(tube(out, Y, 16, 2, false), &rings);
}

fn breast(mesh: &mut Mesh, b: &Build, rest: &Posed, side: f64) {
    let at = rest.at(Joint::Chest) + Vec3::new(0.045 * side * b.shoulders, 0.045 * b.torso, 0.0);
    let out = X.scale(side);
    let k = b.thick;
    let rings: Vec<Ring> = BREAST
        .iter()
        .map(|&(z, outward, inward, up, down, drop)| Ring {
            center: at + Vec3::new(0.0, drop * k, z * k),
            u: (outward * k, inward * k),
            w: (up * k, down * k),
            bones: only(Joint::Chest),
            along: 0.5,
        })
        .collect();
    mesh.tube(tube(out, Y, 16, 0, false), &rings);
}
