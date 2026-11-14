//! Turning a joint about one of its axes on a ring, as a 3D program's rotation handle: a ring
//! for each of bend, twist and side lies round the joint, and a drag along a ring turns the
//! joint about that ring's axis alone, as its slider would.

use crate::figure::Mannequin;
use crate::kinematics::Posed;
use crate::math::{Angles, Rot3, Vec3};
use crate::skeleton::{Joint, Side};
use crate::view::Placement;

/// Points a ring is drawn with.
pub const RING_POINTS: usize = 48;

/// One of the three ways a joint turns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Bend,
    Twist,
    Side,
}

impl Axis {
    pub const ALL: [Axis; 3] = [Self::Bend, Self::Twist, Self::Side];
}

/// A ring round a joint in the picture.
#[derive(Debug, Clone, PartialEq)]
pub struct Ring {
    pub axis: Axis,
    /// [`RING_POINTS`] points of a closed line.
    pub points: Vec<[f64; 2]>,
    /// Whether each point is on the half of the ring toward the viewer.
    pub front: Vec<bool>,
}

/// A hold on a ring.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RingGrab {
    pub joint: Joint,
    pub axis: Axis,
    /// The way along the ring in the picture that turns the joint the positive way.
    along: [f64; 2],
    radius: f64,
}

fn range(joint: Joint, axis: Axis) -> (f64, f64) {
    let limits = joint.limits();
    match axis {
        Axis::Bend => limits.bend,
        Axis::Twist => limits.twist,
        Axis::Side => limits.side,
    }
}

/// On the figure's right the twist and the swing are kept mirrored.
fn sign(joint: Joint, axis: Axis) -> f64 {
    if joint.side() == Side::Right && axis != Axis::Bend {
        -1.0
    } else {
        1.0
    }
}

impl Mannequin {
    /// The line `joint` turns about for `axis`, in the figure's space.
    fn ring_axis(&self, posed: &Posed, joint: Joint, axis: Axis) -> Vec3 {
        let parent = posed.parent_frame(joint);
        match axis {
            Axis::Side => parent.apply(Vec3::new(0.0, 0.0, 1.0)),
            Axis::Bend => {
                let side = self.pose.angles(joint).side * sign(joint, Axis::Side);
                (parent * Rot3::about_z(side)).apply(Vec3::new(1.0, 0.0, 0.0))
            }
            Axis::Twist => posed.frame(joint).apply(Vec3::new(0.0, 1.0, 0.0)),
        }
    }

    /// The rings round `joint`, `radius` picture units across the middle, one for each axis the
    /// joint turns about.
    pub fn rings(&self, joint: Joint, placement: Placement, radius: f64) -> Vec<Ring> {
        let posed = self.posed();
        let projector = self.projector(placement);
        let center = posed.at(joint);
        let seen = projector.project(center);
        let size = radius / (placement.height * seen.scale);
        let viewer = self.view.toward_viewer();
        Axis::ALL
            .into_iter()
            .filter(|axis| {
                let (low, high) = range(joint, *axis);
                low < high
            })
            .map(|axis| {
                let (u, w) = across(self.ring_axis(&posed, joint, axis));
                let (points, front) = (0..RING_POINTS)
                    .map(|i| {
                        let (s, c) =
                            (std::f64::consts::TAU * i as f64 / RING_POINTS as f64).sin_cos();
                        let out = u.scale(c) + w.scale(s);
                        let at = projector.project(center + out.scale(size)).at;
                        (at, out.dot(viewer) >= 0.0)
                    })
                    .unzip();
                Ring {
                    axis,
                    points,
                    front,
                }
            })
            .collect()
    }

    /// Takes hold of the ring of `joint` nearest `press`, within `reach` picture units of it;
    /// the near half of a ring before the far half of another.
    pub fn grab_ring(
        &self,
        joint: Joint,
        placement: Placement,
        radius: f64,
        press: [f64; 2],
        reach: f64,
    ) -> Option<RingGrab> {
        let rings = self.rings(joint, placement, radius);
        let (_, ring, i) = rings
            .iter()
            .flat_map(|ring| {
                ring.points
                    .iter()
                    .enumerate()
                    .map(move |(i, p)| (ring, i, p))
            })
            .map(|(ring, i, p)| {
                let gap = (p[0] - press[0]).hypot(p[1] - press[1]);
                (gap, ring, i)
            })
            .filter(|(gap, ..)| *gap <= reach)
            .min_by(|(a, ra, ia), (b, rb, ib)| {
                rb.front[*ib].cmp(&ra.front[*ia]).then(a.total_cmp(b))
            })?;
        let (before, after) = (
            ring.points[(i + RING_POINTS - 1) % RING_POINTS],
            ring.points[(i + 1) % RING_POINTS],
        );
        let (dx, dy) = (after[0] - before[0], after[1] - before[1]);
        let length = dx.hypot(dy);
        // A ring seen edge on runs straight at the viewer here; across the picture does.
        let along = if length > 1e-6 {
            [dx / length, dy / length]
        } else {
            [1.0, 0.0]
        };
        Some(RingGrab {
            joint,
            axis: ring.axis,
            along,
            radius: radius.max(1.0),
        })
    }

    /// Turns the joint of `grab` for a move of the pointer by `by` in the picture: as far round
    /// its axis as the pointer went along the ring, held to the joint's limits.
    pub fn turn_ring(&mut self, grab: &RingGrab, by: [f64; 2]) {
        let degrees = ((by[0] * grab.along[0] + by[1] * grab.along[1]) / grab.radius).to_degrees();
        if !degrees.is_finite() {
            return;
        }
        let Angles {
            mut bend,
            mut twist,
            mut side,
        } = self.pose.angles(grab.joint);
        let turn = degrees * sign(grab.joint, grab.axis);
        match grab.axis {
            Axis::Bend => bend += turn,
            Axis::Twist => twist += turn,
            Axis::Side => side += turn,
        }
        self.pose.set(grab.joint, Angles::new(bend, twist, side));
    }
}

/// Two directions across `along`, the three at right angles and right-handed.
fn across(along: Vec3) -> (Vec3, Vec3) {
    let other = if along.x.abs() < 0.9 {
        Vec3::new(1.0, 0.0, 0.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let u = along
        .cross(other)
        .normalized()
        .unwrap_or(Vec3::new(0.0, 0.0, 1.0));
    (u, along.cross(u))
}
