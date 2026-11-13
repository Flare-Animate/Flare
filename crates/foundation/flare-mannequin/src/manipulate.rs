//! Posing by turning and reaching: a part turned about the line of sight, as if it were pushed
//! round on the glass, or a hand or a foot pulled to a point with the arm or leg following
//! (`grab` builds dragging any part on these, `rings` turning about one axis). The reach is a
//! two-bone solve in the figure's space, at the depth the hand or foot already has: the elbow or
//! knee bends the one way it can by the law of cosines, then the shoulder or hip swings the limb
//! onto the point, every joint held to its limits. The view never changes here.

use crate::figure::Mannequin;
use crate::math::{Angles, Rot3, Vec3, wrap_degrees};
use crate::skeleton::{Joint, Side};
use crate::view::Placement;

/// Most rounds of a reach before it settles for where it got (a limit can stop it short).
pub const MAX_REACH_ITERATIONS: usize = 8;
/// A reach this close to its point, in picture units, is there.
const REACH_TOLERANCE: f64 = 0.25;
/// A reach this close to a point of the figure's space is there (a hundredth of a percent of
/// the figure's height).
const POINT_TOLERANCE: f64 = 1e-4;
/// How straight a limb is let go at full stretch, so the elbow or knee keeps the side it bends to.
const NEARLY_STRAIGHT: f64 = 0.9995;

fn screen_angle(from: [f64; 2], to: [f64; 2]) -> Option<f64> {
    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
    (dx.hypot(dy) > 1e-6).then(|| dy.atan2(dx).to_degrees())
}

/// How far `angles` are outside `joint`'s limits, in degrees all told.
fn excess(joint: Joint, angles: Angles) -> f64 {
    if joint == Joint::Pelvis {
        return 0.0;
    }
    let held = joint.limits().clamp(angles);
    (angles.bend - held.bend).abs()
        + (angles.twist - held.twist).abs()
        + (angles.side - held.side).abs()
}

fn distance(a: Angles, b: Angles) -> f64 {
    wrap_degrees(a.bend - b.bend).abs()
        + wrap_degrees(a.twist - b.twist).abs()
        + wrap_degrees(a.side - b.side).abs()
}

/// The angles, as the pose keeps them (mirrored on the right), of `rotation` for `joint`: of the
/// two sets that make it, the one inside the limits, else the one nearest `was`, so an arm raised
/// overhead is not read as one turned round.
fn angles_for(joint: Joint, rotation: Rot3, was: Angles) -> Angles {
    let a = rotation.to_angles();
    let b = Angles::new(
        wrap_degrees(180.0 - a.bend),
        wrap_degrees(a.twist + 180.0),
        wrap_degrees(a.side + 180.0),
    );
    let a = Angles::new(a.bend, wrap_degrees(a.twist), wrap_degrees(a.side));
    let kept = |c: Angles| {
        if joint.side() == Side::Right {
            c.mirrored()
        } else {
            c
        }
    };
    let (a, b) = (kept(a), kept(b));
    let cost = |c: Angles| excess(joint, c) * 1000.0 + distance(c, was);
    if cost(b) < cost(a) { b } else { a }
}

fn gap(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

impl Mannequin {
    /// Turns `joint`'s frame by `turn`, given in the figure's space, as far as its limits allow.
    pub(crate) fn turn_frame(&mut self, joint: Joint, turn: Rot3) {
        let posed = self.posed();
        let world = turn * posed.frame(joint);
        let local = posed.parent_frame(joint).transpose() * world;
        let angles = angles_for(joint, local, self.pose.angles(joint));
        self.pose.set(joint, angles);
    }

    /// Turns `joint` by `degrees` about the line of sight, counterclockwise as seen.
    pub fn turn_by(&mut self, joint: Joint, degrees: f64) {
        if degrees.is_finite() {
            self.turn_frame(joint, Rot3::about_axis(self.view.toward_viewer(), degrees));
        }
    }

    /// A drag from `from` to `to` in the picture at `placement`: turns `joint` round its own place
    /// by the angle the pointer swept.
    pub fn turn(&mut self, joint: Joint, placement: Placement, from: [f64; 2], to: [f64; 2]) {
        let pivot = self.marks(placement)[joint.index()].at;
        if let (Some(a), Some(b)) = (screen_angle(pivot, from), screen_angle(pivot, to)) {
            // The picture's y runs down, so a sweep that looks counterclockwise is negative.
            self.turn_by(joint, -wrap_degrees(b - a));
        }
    }

    /// Pulls a hand or a foot (`effector`, a wrist or an ankle) toward `target` in the picture,
    /// bending its arm or leg. Whether it got there; out of reach, the limb points at it.
    pub fn reach(&mut self, effector: Joint, placement: Placement, target: [f64; 2]) -> bool {
        if effector.reach_chain().is_none() || !(target[0].is_finite() && target[1].is_finite()) {
            return false;
        }
        let projector = self.projector(placement);
        let depth = self.marks(placement)[effector.index()].depth;
        let goal = projector.unproject(target, depth);
        self.reach_goal(effector, goal, |m| {
            gap(m.marks(placement)[effector.index()].at, target) <= REACH_TOLERANCE
        })
    }

    /// Pulls a hand or a foot to `goal`, a point of the figure's space (x to its left, y up
    /// from the ground, z to its front, the default figure one unit tall). Whether it got there.
    pub fn reach_point(&mut self, effector: Joint, goal: Vec3) -> bool {
        if effector.reach_chain().is_none() || !goal.is_finite() {
            return false;
        }
        self.reach_goal(effector, goal, |m| {
            (m.posed().at(effector) - goal).length() <= POINT_TOLERANCE
        })
    }

    fn reach_goal(&mut self, effector: Joint, goal: Vec3, there: impl Fn(&Self) -> bool) -> bool {
        let Some([hinge, root]) = effector.reach_chain() else {
            return false;
        };
        let bends_forward = hinge.limits().bend.1 <= 0.0;
        for _ in 0..MAX_REACH_ITERATIONS {
            let posed = self.posed();
            let (s, e, w) = (posed.at(root), posed.at(hinge), posed.at(effector));
            let (upper, lower) = ((e - s).length(), (w - e).length());
            let want = (goal - s).length().clamp(
                (upper - lower).abs() + 1e-6,
                (upper + lower) * NEARLY_STRAIGHT,
            );
            // The bend that puts the end `want` from the root.
            let cos = ((upper * upper + lower * lower - want * want) / (2.0 * upper * lower))
                .clamp(-1.0, 1.0);
            let bend = 180.0 - cos.acos().to_degrees();
            let mut angles = self.pose.angles(hinge);
            angles.bend = if bends_forward { -bend } else { bend };
            self.pose.set(hinge, angles);
            // Then swing the whole limb from its root onto the point.
            let posed = self.posed();
            let (s, w) = (posed.at(root), posed.at(effector));
            if let (Some(now), Some(wanted)) = ((w - s).normalized(), (goal - s).normalized()) {
                self.turn_frame(root, Rot3::between(now, wanted));
            }
            if there(self) {
                return true;
            }
        }
        false
    }

    /// Turns the view: `yaw` more degrees about the figure's vertical, `pitch` more from above.
    pub fn orbit(&mut self, yaw: f64, pitch: f64) {
        let (y, p) = (self.view.yaw() + yaw, self.view.pitch() + pitch);
        self.view.set_yaw(y);
        self.view.set_pitch(p);
    }
}
