//! A picture to pose the figure from: marks on it where a person's head, neck, shoulders,
//! elbows, hands, hips, knees and feet are, and where the picture stands in the figure's space
//! (a board facing the front, the way the figure is seen from the front). The pose is worked out
//! from the marks by `copy` (`Mannequin::copy_pose`). The marks are in the picture's own units,
//! x to the right and y down, so they read the same whatever size the picture is shown at.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::figure::Mannequin;
use crate::kinematics::Posed;
use crate::math::Vec3;
use crate::skeleton::{JOINT_COUNT, Joint};
use crate::view::Projector;

/// The parts that are marked on a picture. The head's mark is the middle of the head; the
/// others are the joints themselves (the neck's sits between the collarbones).
pub const MARKED: [Joint; 14] = [
    Joint::Head,
    Joint::Neck,
    Joint::LeftShoulder,
    Joint::LeftElbow,
    Joint::LeftWrist,
    Joint::RightShoulder,
    Joint::RightElbow,
    Joint::RightWrist,
    Joint::LeftHip,
    Joint::LeftKnee,
    Joint::LeftAnkle,
    Joint::RightHip,
    Joint::RightKnee,
    Joint::RightAnkle,
];

/// The lines between marks, which show them as a stick figure.
pub const MARK_LINES: [(Joint, Joint); 13] = [
    (Joint::Head, Joint::Neck),
    (Joint::LeftShoulder, Joint::RightShoulder),
    (Joint::LeftShoulder, Joint::LeftElbow),
    (Joint::LeftElbow, Joint::LeftWrist),
    (Joint::RightShoulder, Joint::RightElbow),
    (Joint::RightElbow, Joint::RightWrist),
    (Joint::LeftHip, Joint::RightHip),
    (Joint::LeftHip, Joint::LeftKnee),
    (Joint::LeftKnee, Joint::LeftAnkle),
    (Joint::RightHip, Joint::RightKnee),
    (Joint::RightKnee, Joint::RightAnkle),
    (Joint::Neck, Joint::LeftHip),
    (Joint::Neck, Joint::RightHip),
];

/// The furthest a mark may be from the picture's corner, in its units.
pub const MAX_MARK: f64 = 1e6;
/// The largest marks file read.
pub const MAX_MARKS_BYTES: usize = 8 * 1024;
/// How high the middle of the head is above the head joint, as a share of the head's size.
pub(crate) const HEAD_MIDDLE: f64 = 0.06;

/// Whether a marked part is nearer the viewer than the part it hangs from, which a flat picture
/// cannot say: worked out from how a body bends, or as the person says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Depth {
    #[default]
    Auto,
    Nearer,
    Farther,
}

impl Depth {
    pub fn id(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Nearer => "nearer",
            Self::Farther => "farther",
        }
    }

    pub fn from_id(id: &str) -> Option<Depth> {
        [Self::Auto, Self::Nearer, Self::Farther]
            .into_iter()
            .find(|d| d.id() == id)
    }

    /// The next one round, as a click on a mark steps through them.
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Nearer,
            Self::Nearer => Self::Farther,
            Self::Farther => Self::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum MarksError {
    #[error("a marks file is limited to {MAX_MARKS_BYTES} bytes")]
    TooLarge,
    #[error("the marks cannot be read: {0}")]
    Json(String),
    #[error("{0:?} is not a part that is marked; the marks are {list}", list = marked_list())]
    NotMarked(String),
    #[error("the mark of {0} must be two numbers within {MAX_MARK} of the picture's corner")]
    BadPoint(&'static str),
    #[error("{0:?} is not a depth; a depth is auto, nearer or farther")]
    BadDepth(String),
    #[error("a pose needs the marks of both hips and the neck, spread apart")]
    TooFew,
}

fn marked_list() -> String {
    MARKED.map(Joint::id).join(", ")
}

/// Where each marked part is on the picture, and which way it goes in depth.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Marks {
    at: [Option<[f64; 2]>; JOINT_COUNT],
    depth: [Depth; JOINT_COUNT],
}

fn is_marked(joint: Joint) -> bool {
    MARKED.contains(&joint)
}

fn good(at: [f64; 2]) -> bool {
    at.iter().all(|v| v.is_finite() && v.abs() <= MAX_MARK)
}

impl Marks {
    pub fn get(&self, joint: Joint) -> Option<[f64; 2]> {
        self.at[joint.index()]
    }

    /// Puts `joint`'s mark at `at`, or takes it off with none. Whether it was taken: a part that
    /// is not marked, or a point that is not a number or is too far, is refused.
    pub fn set(&mut self, joint: Joint, at: Option<[f64; 2]>) -> bool {
        if !is_marked(joint) || at.is_some_and(|p| !good(p)) {
            return false;
        }
        self.at[joint.index()] = at;
        true
    }

    pub fn depth(&self, joint: Joint) -> Depth {
        self.depth[joint.index()]
    }

    pub fn set_depth(&mut self, joint: Joint, depth: Depth) {
        if is_marked(joint) {
            self.depth[joint.index()] = depth;
        }
    }

    /// The mark nearest `at`, if one is within `reach`.
    pub fn nearest(&self, at: [f64; 2], reach: f64) -> Option<Joint> {
        MARKED
            .into_iter()
            .filter_map(|j| self.get(j).map(|p| (j, (p[0] - at[0]).hypot(p[1] - at[1]))))
            .filter(|(_, d)| *d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(j, _)| j)
    }

    /// Marks where `figure`'s parts lie on `board`, seen from the front: a start to drag onto
    /// the person in the picture.
    pub fn from_figure(figure: &Mannequin, board: &Board) -> Self {
        let posed = figure.posed();
        let head = figure.build().head;
        let mut marks = Self::default();
        for joint in MARKED {
            let p = marked_point(&posed, head, joint);
            marks.at[joint.index()] = Some(board.picture_point(p));
        }
        marks
    }

    /// The marks as JSON: `{"marks": {"left_elbow": [x, y], ...}, "depth": {"left_elbow":
    /// "nearer"}}`, with x to the right and y down in the picture's units.
    pub fn to_json(&self) -> String {
        let round = |v: f64| (v * 100.0).round() / 100.0;
        let wire = MarksWire {
            marks: MARKED
                .into_iter()
                .filter_map(|j| self.get(j).map(|p| (j.id().to_owned(), p.map(round))))
                .collect(),
            depth: MARKED
                .into_iter()
                .filter(|j| self.depth(*j) != Depth::Auto)
                .map(|j| (j.id().to_owned(), self.depth(j).id().to_owned()))
                .collect(),
        };
        serde_json::to_string(&wire).unwrap_or_default()
    }

    /// Reads marks written by [`to_json`](Self::to_json); a part left out is not marked.
    pub fn from_json(text: &str) -> Result<Self, MarksError> {
        if text.len() > MAX_MARKS_BYTES {
            return Err(MarksError::TooLarge);
        }
        let wire: MarksWire =
            serde_json::from_str(text).map_err(|e| MarksError::Json(e.to_string()))?;
        let mut marks = Self::default();
        let joint = |name: &str| {
            Joint::from_id(name)
                .filter(|j| is_marked(*j))
                .ok_or_else(|| MarksError::NotMarked(name.to_owned()))
        };
        for (name, at) in &wire.marks {
            let joint = joint(name)?;
            if !marks.set(joint, Some(*at)) {
                return Err(MarksError::BadPoint(joint.id()));
            }
        }
        for (name, depth) in &wire.depth {
            let joint = joint(name)?;
            let depth = Depth::from_id(depth).ok_or_else(|| MarksError::BadDepth(depth.clone()))?;
            marks.set_depth(joint, depth);
        }
        Ok(marks)
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MarksWire {
    #[serde(default)]
    marks: BTreeMap<String, [f64; 2]>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    depth: BTreeMap<String, String>,
}

/// Where `joint`'s mark belongs on a figure posed as `posed` with a head `head` large, in the
/// figure's space.
pub(crate) fn marked_point(posed: &Posed, head: f64, joint: Joint) -> Vec3 {
    if joint == Joint::Head {
        posed.point(Joint::Head, Vec3::new(0.0, HEAD_MIDDLE * head, 0.0))
    } else {
        posed.at(joint)
    }
}

/// Where a picture stands in the figure's space: upright and facing the front, its top left
/// corner at `origin`, one unit of the picture `unit` figure units long.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Board {
    origin: Vec3,
    unit: f64,
    size: [f64; 2],
}

/// How tall a picture first stands, in figure units: a little taller than the figure.
const FIRST_HEIGHT: f64 = 1.2;

impl Board {
    /// A picture `size` across standing behind the figure, a little taller than it, its middle
    /// over the figure's middle; none for a size that is not a picture's.
    pub fn new(size: [f64; 2]) -> Option<Self> {
        if !(size
            .iter()
            .all(|v| v.is_finite() && *v >= 1.0 && *v <= MAX_MARK))
        {
            return None;
        }
        let unit = FIRST_HEIGHT / size[1];
        Some(Self {
            origin: Vec3::new(-size[0] * unit / 2.0, FIRST_HEIGHT - 0.1, 0.0),
            unit,
            size,
        })
    }

    /// The picture moved and sized so `picture` lands on `figure` (a point of the figure's
    /// space) and one figure unit is `scale` of the picture's units, as a copied pose asks.
    pub fn fitted(size: [f64; 2], fit: &Fit) -> Option<Self> {
        let board = Self::new(size)?;
        if !(fit.scale.is_finite() && fit.scale > 1e-9) {
            return None;
        }
        let unit = 1.0 / fit.scale;
        let origin = Vec3::new(
            fit.figure.x - fit.picture[0] * unit,
            fit.figure.y + fit.picture[1] * unit,
            fit.figure.z,
        );
        origin.is_finite().then_some(Self {
            origin,
            unit,
            ..board
        })
    }

    pub fn size(&self) -> [f64; 2] {
        self.size
    }

    /// The point of the figure's space at `at` on the picture.
    pub fn point(&self, at: [f64; 2]) -> Vec3 {
        Vec3::new(
            self.origin.x + at[0] * self.unit,
            self.origin.y - at[1] * self.unit,
            self.origin.z,
        )
    }

    /// Where `p` lies on the picture, seen square from the front.
    pub fn picture_point(&self, p: Vec3) -> [f64; 2] {
        [
            (p.x - self.origin.x) / self.unit,
            (self.origin.y - p.y) / self.unit,
        ]
    }

    /// The picture's corners in the figure's space: top left, top right, bottom right, bottom
    /// left.
    pub fn corners(&self) -> [Vec3; 4] {
        let [w, h] = self.size;
        [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]].map(|at| self.point(at))
    }

    /// The point of the picture seen at `at` through `projector`; none where the picture is
    /// seen edge on, or behind the camera.
    pub fn under(&self, projector: &Projector, at: [f64; 2]) -> Option<[f64; 2]> {
        // Two points the line of sight through `at` passes, and where it meets the board.
        let (a, b) = (projector.unproject(at, 0.0), projector.unproject(at, -1.0));
        let dz = b.z - a.z;
        if dz.abs() < 1e-6 {
            return None;
        }
        let t = (self.origin.z - a.z) / dz;
        let p = a + (b - a).scale(t);
        let seen = self.picture_point(p);
        (projector.in_front(projector.project(p).depth) && good(seen)).then_some(seen)
    }
}

/// How a copied pose lines up with its picture: `picture` (a point on it, in its units) is
/// where `figure` (the same point of the figure's space) is, and one figure unit is `scale`
/// of the picture's units.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub picture: [f64; 2],
    pub figure: Vec3,
    pub scale: f64,
}
