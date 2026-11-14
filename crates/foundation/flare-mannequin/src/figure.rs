//! The mannequin: a body, its proportions, its pose and the view of it, and where it lands in a
//! picture.

use crate::body::{Body, Build, Proportions};
use crate::kinematics::{Posed, pose_figure};
use crate::math::Vec3;
use crate::pose::Pose;
use crate::shape::body;
use crate::skeleton::{JOINT_COUNT, Joint};
use crate::skin::{Mesh, Skinned, skin};
use crate::view::{Placement, Projector, View};

/// A posable figure for reference.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Mannequin {
    pub body: Body,
    pub proportions: Proportions,
    pub pose: Pose,
    pub view: View,
}

/// Where a joint lands in the picture.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JointMark {
    pub joint: Joint,
    pub at: [f64; 2],
    /// Larger is nearer the viewer.
    pub depth: f64,
}

/// The side of a square of the floor's grid, and how many squares it runs each way.
const FLOOR_STEP: f64 = 0.2;
const FLOOR_SQUARES: i32 = 3;

/// The way the light that casts the shadow on the floor falls, in the figure's space: from
/// above, a little in front and to the figure's right.
pub(crate) const SHADOW_FALL: Vec3 = Vec3::new(0.12, -1.0, -0.22);

impl Mannequin {
    pub fn new(body: Body) -> Self {
        Self {
            body,
            ..Self::default()
        }
    }

    pub(crate) fn build(&self) -> Build {
        Build::new(self.body, &self.proportions)
    }

    /// Where every joint is and which way it faces.
    pub fn posed(&self) -> Posed {
        pose_figure(&self.build(), &self.pose)
    }

    /// Turns points of the figure's space into points of the picture at `placement`.
    pub fn projector(&self, placement: Placement) -> Projector {
        Projector::new(&self.view, placement)
    }

    /// The body's surface at rest, and where the pose puts it.
    pub(crate) fn surface(&self) -> (Mesh, Skinned) {
        let build = self.build();
        let rest = pose_figure(&build, &Pose::default());
        let posed = pose_figure(&build, &self.pose);
        let mesh = body(&build, &rest);
        let skinned = skin(&mesh, &rest, &posed);
        (mesh, skinned)
    }

    /// Where each joint lands in the picture.
    pub fn marks(&self, placement: Placement) -> [JointMark; JOINT_COUNT] {
        let posed = self.posed();
        let projector = self.projector(placement);
        Joint::ALL.map(|joint| {
            let p = projector.project(posed.at(joint));
            JointMark {
                joint,
                at: p.at,
                depth: p.depth,
            }
        })
    }

    /// The smallest box round the figure in the picture at `placement`, and round its shadow
    /// too when `shadow` is asked for, as its least and greatest corners.
    pub fn bounds(&self, placement: Placement, shadow: bool) -> ([f64; 2], [f64; 2]) {
        let (_, skinned) = self.surface();
        let projector = self.projector(placement);
        let mut low = [f64::MAX; 2];
        let mut high = [f64::MIN; 2];
        let mut take = |p: Vec3| {
            let at = projector.project(p).at;
            for k in 0..2 {
                low[k] = low[k].min(at[k]);
                high[k] = high[k].max(at[k]);
            }
        };
        for p in &skinned.points {
            take(*p);
            if shadow {
                take(on_floor(*p));
            }
        }
        (low, high)
    }

    /// A grid on the ground where the figure first stood, as lines in the same picture, so a
    /// figure that is moved or lifted is seen to leave its place.
    pub fn floor(&self, placement: Placement) -> Vec<[[f64; 2]; 2]> {
        let projector = self.projector(placement);
        let at = |x: f64, z: f64| projector.project(Vec3::new(x, 0.0, z)).at;
        let reach = FLOOR_STEP * f64::from(FLOOR_SQUARES);
        (-FLOOR_SQUARES..=FLOOR_SQUARES)
            .flat_map(|i| {
                let k = f64::from(i) * FLOOR_STEP;
                [[at(k, -reach), at(k, reach)], [at(-reach, k), at(reach, k)]]
            })
            .collect()
    }
}

/// Where the shadow of `p` falls on the floor.
pub(crate) fn on_floor(p: Vec3) -> Vec3 {
    let height = p.y.max(0.0);
    Vec3::new(
        p.x + SHADOW_FALL.x * height / -SHADOW_FALL.y,
        0.0,
        p.z + SHADOW_FALL.z * height / -SHADOW_FALL.y,
    )
}
