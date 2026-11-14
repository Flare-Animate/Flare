//! Copying a pose from marks on a picture, as if the picture were the figure seen square from
//! the front. A flat picture loses depth; the figure's own lengths give it back: a bone that
//! looks shorter than it is leans toward or away from the viewer by just enough (the length
//! seen, against the length it is, by Pythagoras). A bone that looks only a little short is
//! taken to lie in the picture's plane (up to [`LEAN_SLACK`]), since a person is not built quite
//! as the figure is and a mark found in a photograph is a little off; a part the person gave a
//! depth for leans exactly as far as it looks. Which way it leans the picture cannot say,
//! so both ways are tried, and the one a body can make (every joint inside its limits, the
//! nearest to the pose the figure had when the two are as good) is kept, unless the person said
//! which way on the mark. The scale is the one at which no bone is longer than it can be.
//!
//! Then the body is posed onto those lines, from the hips out: the hips and the line up to the
//! neck turn the whole figure, the shoulders the waist and chest; each arm and leg swings its
//! upper bone onto its line, bends its elbow or knee to the angle between its two lines and
//! turns about the upper bone until the lower one lies on its line too. The feet are put back
//! on the floor.

use crate::body::ANKLE_HEIGHT;
use crate::figure::Mannequin;
use crate::kinematics::{Posed, pose_figure};
use crate::manipulate::angles_for;
use crate::math::{Rot3, Vec3, wrap_degrees};
use crate::pose::Pose;
use crate::reference::{Depth, Fit, Marks, MarksError, marked_point};
use crate::skeleton::Joint;

/// One end of a line between marks: a mark, or halfway between the hips.
#[derive(Debug, Clone, Copy, PartialEq)]
enum End {
    Mark(Joint),
    Hips,
}

/// A line of the body between two marks, and the mark whose depth says which way it leans
/// (with `flip` when that mark nearer means the line leans away).
#[derive(Debug, Clone, Copy)]
struct Line {
    from: End,
    to: End,
    says: [(Joint, bool); 2],
    /// Long enough to tell the scale by: the lengths of the hips, shoulders and neck differ
    /// most from person to person.
    long: bool,
}

const fn mark(joint: Joint) -> End {
    End::Mark(joint)
}

const fn line(from: Joint, to: Joint) -> Line {
    Line {
        from: mark(from),
        to: mark(to),
        says: [(to, false), (to, false)],
        long: true,
    }
}

const HIPS: Line = Line {
    from: mark(Joint::RightHip),
    to: mark(Joint::LeftHip),
    says: [(Joint::LeftHip, false), (Joint::RightHip, true)],
    long: false,
};
const TORSO: Line = Line {
    from: End::Hips,
    to: mark(Joint::Neck),
    says: [(Joint::Neck, false), (Joint::Neck, false)],
    long: true,
};
const SHOULDERS: Line = Line {
    from: mark(Joint::RightShoulder),
    to: mark(Joint::LeftShoulder),
    says: [(Joint::LeftShoulder, false), (Joint::RightShoulder, true)],
    long: false,
};
const NECK: Line = Line {
    long: false,
    ..line(Joint::Neck, Joint::Head)
};

/// An arm or a leg: the joint it hangs from, its elbow or knee, its hand or foot.
const LIMBS: [[Joint; 3]; 4] = [
    [Joint::LeftShoulder, Joint::LeftElbow, Joint::LeftWrist],
    [Joint::RightShoulder, Joint::RightElbow, Joint::RightWrist],
    [Joint::LeftHip, Joint::LeftKnee, Joint::LeftAnkle],
    [Joint::RightHip, Joint::RightKnee, Joint::RightAnkle],
];

/// A line as the picture shows it, with the figure's length for it.
#[derive(Debug, Clone, Copy)]
struct Seen {
    line: Line,
    /// Across and up, in picture units (up is the picture's y turned over).
    flat: [f64; 2],
    length: f64,
}

/// A line put back into depth: its direction in the figure's space.
type Lifted = Option<Vec3>;

/// The marks read against the figure: each line seen, and the scale.
struct Reading {
    seen: Vec<Seen>,
    scale: f64,
    marks: Marks,
}

/// An elbow or knee bent less than this, in degrees, does not turn its limb about the upper
/// bone.
const TWIST_BEND: f64 = 15.0;
/// Weight of the tie-break toward the pose the figure had, per degree.
const NEAREST: f64 = 1e-6;
/// A line leaning less than this, in degrees, lies flat in the picture and leans neither way.
const FLAT: f64 = 0.06;
/// How far a line may seem to lean out of the picture, in degrees, before it is taken to: the
/// shortening a different build or a mark a little off makes on a bone in the picture's plane.
pub const LEAN_SLACK: f64 = 25.0;

fn picture_point(marks: &Marks, end: End) -> Option<[f64; 2]> {
    match end {
        End::Mark(joint) => marks.get(joint),
        End::Hips => {
            let (a, b) = (marks.get(Joint::LeftHip)?, marks.get(Joint::RightHip)?);
            Some([(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0])
        }
    }
}

fn figure_point(posed: &Posed, head: f64, end: End) -> Vec3 {
    match end {
        End::Mark(joint) => marked_point(posed, head, joint),
        End::Hips => (posed.at(Joint::LeftHip) + posed.at(Joint::RightHip)).scale(0.5),
    }
}

fn direction(posed: &Posed, head: f64, line: &Line) -> Option<Vec3> {
    (figure_point(posed, head, line.to) - figure_point(posed, head, line.from)).normalized()
}

impl Reading {
    fn new(figure: &Mannequin, marks: &Marks) -> Result<Self, MarksError> {
        let build = figure.build();
        let rest = pose_figure(&build, &Pose::default());
        let mut lines = vec![HIPS, TORSO, SHOULDERS, NECK];
        for [root, hinge, end] in LIMBS {
            lines.push(line(root, hinge));
            lines.push(line(hinge, end));
        }
        let seen: Vec<Seen> = lines
            .into_iter()
            .filter_map(|line| {
                let (a, b) = (
                    picture_point(marks, line.from)?,
                    picture_point(marks, line.to)?,
                );
                let length = (figure_point(&rest, build.head, line.to)
                    - figure_point(&rest, build.head, line.from))
                .length();
                Some(Seen {
                    line,
                    flat: [b[0] - a[0], a[1] - b[1]],
                    length,
                })
            })
            .collect();
        let hips = seen
            .iter()
            .any(|s| matches!(s.line.to, End::Mark(Joint::Neck)));
        let ratio = |s: &Seen| s.flat[0].hypot(s.flat[1]) / s.length;
        let long = seen
            .iter()
            .filter(|s| s.line.long)
            .map(ratio)
            .fold(0.0, f64::max);
        let scale = if long > 0.0 {
            long
        } else {
            seen.iter().map(ratio).fold(0.0, f64::max)
        };
        if !(hips && scale.is_finite() && scale > 1e-9) {
            return Err(MarksError::TooFew);
        }
        Ok(Self {
            seen,
            scale,
            marks: *marks,
        })
    }

    fn find(&self, line: &Line) -> Option<&Seen> {
        self.seen
            .iter()
            .find(|s| s.line.from == line.from && s.line.to == line.to)
    }

    /// The ways `line` may lean: toward the viewer (+1) or away (-1). One way for a line flat in
    /// the picture or one the person gave a way for; none for a line that is not marked.
    fn ways(&self, line: &Line) -> Vec<f64> {
        let Some(seen) = self.find(line) else {
            return vec![];
        };
        if self.lean(seen) < FLAT {
            return vec![1.0];
        }
        match self.told(line) {
            Some(sign) => vec![sign],
            None => vec![1.0, -1.0],
        }
    }

    /// Which way the person said `line` leans: toward the viewer (+1) or away (-1).
    fn told(&self, line: &Line) -> Option<f64> {
        line.says.iter().find_map(|(joint, flip)| {
            let sign = if *flip { -1.0 } else { 1.0 };
            match self.marks.depth(*joint) {
                Depth::Nearer => Some(sign),
                Depth::Farther => Some(-sign),
                Depth::Auto => None,
            }
        })
    }

    /// How far a line leans out of the picture, in degrees: the angle whose cosine is how much
    /// shorter it looks than it is. A line the person gave a way for leans just that far; one
    /// left to work out leans only past [`LEAN_SLACK`], the rest of the way stretched to a
    /// quarter turn, so a bone that only looks a little short (a person not built as the
    /// figure is, a mark a little off) stays in the picture's plane.
    fn lean(&self, seen: &Seen) -> f64 {
        let [x, y] = seen.flat.map(|v| v / self.scale);
        let looks = (x.hypot(y) / seen.length).clamp(0.0, 1.0);
        let lean = looks.acos().to_degrees();
        if self.told(&seen.line).is_some() {
            lean
        } else {
            ((lean - LEAN_SLACK) * 90.0 / (90.0 - LEAN_SLACK)).max(0.0)
        }
    }

    /// `line` leaning `way`, as a direction of the figure's space.
    fn lift(&self, line: &Line, way: f64) -> Lifted {
        let seen = self.find(line)?;
        let (sin, cos) = self.lean(seen).to_radians().sin_cos();
        let across = Vec3::new(seen.flat[0], seen.flat[1], 0.0)
            .normalized()
            .unwrap_or(Vec3::ZERO);
        (across.scale(cos) + Vec3::new(0.0, 0.0, way * sin)).normalized()
    }
}

/// The frame whose x runs along `across` and whose y is as near `up` as that allows.
fn frame(across: Vec3, up: Vec3) -> Option<Rot3> {
    let x = across.normalized()?;
    let y = (up - x.scale(up.dot(x))).normalized()?;
    let z = x.cross(y);
    Some(Rot3([[x.x, y.x, z.x], [x.y, y.y, z.y], [x.z, y.z, z.z]]))
}

/// How far `posed` is from the directions wanted, line by line (0 is on every one).
fn miss(posed: &Posed, head: f64, wanted: &[(Line, Vec3)]) -> f64 {
    wanted
        .iter()
        .map(|(line, want)| direction(posed, head, line).map_or(2.0, |d| 1.0 - d.dot(*want)))
        .sum()
}

/// How far `joints` turned from `start`, in degrees all told, which breaks a tie.
fn moved(pose: &Pose, start: &Pose, joints: &[Joint]) -> f64 {
    joints
        .iter()
        .map(|j| {
            let (a, b) = (pose.angles(*j), start.angles(*j));
            wrap_degrees(a.bend - b.bend).abs()
                + wrap_degrees(a.twist - b.twist).abs()
                + wrap_degrees(a.side - b.side).abs()
        })
        .sum()
}

impl Mannequin {
    fn set_local(&mut self, joint: Joint, rotation: Rot3) {
        let angles = angles_for(joint, rotation, self.pose.angles(joint));
        self.pose.set(joint, angles);
    }

    /// The hips and the line up to the neck turn the whole figure; the shoulders, against
    /// them, the waist and the chest, half of each angle each (the two make the whole turn
    /// exactly when it is about one axis, as a twist of the shoulders against the hips is).
    fn copy_torso(&mut self, hips: Vec3, torso: Vec3, shoulders: Lifted) {
        let Some(pelvis) = frame(hips, torso) else {
            return;
        };
        self.set_local(Joint::Pelvis, pelvis);
        let half = shoulders
            .and_then(|s| frame(s, torso))
            .map_or(crate::Angles::ZERO, |chest| {
                let whole = (pelvis.transpose() * chest).to_angles();
                crate::Angles::new(whole.bend / 2.0, whole.twist / 2.0, whole.side / 2.0)
            });
        for joint in [Joint::Spine, Joint::Chest] {
            self.pose.set(joint, half);
        }
    }

    /// The neck leans onto its line, and the head takes what the neck's limits leave.
    fn copy_neck(&mut self, want: Vec3) {
        self.pose.set(Joint::Head, crate::Angles::ZERO);
        let head = self.build().head;
        for joint in [Joint::Neck, Joint::Head] {
            let posed = self.posed();
            if let Some(now) = direction(&posed, head, &NECK) {
                self.turn_frame(joint, Rot3::between(now, want));
            }
        }
    }

    /// An arm or a leg onto its two lines: the upper bone swung onto the first, the elbow or
    /// knee bent to the angle between them, then the limb turned about the upper bone until the
    /// lower one lies on the second.
    fn copy_limb(&mut self, [root, hinge, end]: [Joint; 3], upper: Vec3, lower: Lifted) {
        for _ in 0..2 {
            let posed = self.posed();
            if let Some(now) = (posed.at(hinge) - posed.at(root)).normalized() {
                self.turn_frame(root, Rot3::between(now, upper));
            }
            let Some(lower) = lower else {
                return;
            };
            let bend = upper.dot(lower).clamp(-1.0, 1.0).acos().to_degrees();
            let mut angles = crate::Angles::ZERO;
            angles.bend = if hinge.limits().bend.1 <= 0.0 {
                -bend
            } else {
                bend
            };
            self.pose.set(hinge, angles);
            let posed = self.posed();
            let (Some(bone), Some(now)) = (
                (posed.at(hinge) - posed.at(root)).normalized(),
                (posed.at(end) - posed.at(hinge)).normalized(),
            ) else {
                return;
            };
            // A limb nearly straight bends in no plane the marks can tell: turning it to the
            // plane of a slight bend would twist it as far as it goes for nothing.
            if bend < TWIST_BEND {
                continue;
            }
            let across = |v: Vec3| (v - bone.scale(v.dot(bone))).normalized();
            if let (Some(a), Some(b)) = (across(now), across(lower)) {
                let turn = bone.dot(a.cross(b)).atan2(a.dot(b)).to_degrees();
                self.turn_frame(root, Rot3::about_axis(bone, turn));
            }
        }
    }

    /// Poses the figure from `marks` on a picture of it seen from the front: the marks of both
    /// hips and the neck at least, and of whatever else shows. The parts without marks keep
    /// their pose; the hands, feet and head are not turned. Says how the figure now lines up
    /// with the picture.
    pub fn copy_pose(&mut self, marks: &Marks) -> Result<Fit, MarksError> {
        let reading = Reading::new(self, marks)?;
        let start = self.pose;
        let head = self.build().head;
        self.pose.set_lift(Vec3::ZERO);
        let lifted = |line: &Line, way: f64| reading.lift(line, way);
        let one = |line: &Line| reading.ways(line);
        let mut best: Option<(f64, Mannequin)> = None;
        for hips in one(&HIPS) {
            for torso in one(&TORSO) {
                let shoulders = one(&SHOULDERS);
                let shoulders: Vec<Option<f64>> = if shoulders.is_empty() {
                    vec![None]
                } else {
                    shoulders.into_iter().map(Some).collect()
                };
                for way in shoulders {
                    let (Some(h), Some(t)) = (lifted(&HIPS, hips), lifted(&TORSO, torso)) else {
                        return Err(MarksError::TooFew);
                    };
                    let s = way.and_then(|w| lifted(&SHOULDERS, w));
                    let mut trial = *self;
                    trial.copy_torso(h, t, s);
                    let mut wanted = vec![(HIPS, h), (TORSO, t)];
                    wanted.extend(s.map(|s| (SHOULDERS, s)));
                    let mut score = miss(&trial.posed(), head, &wanted)
                        + NEAREST
                            * moved(
                                &trial.pose,
                                &start,
                                &[Joint::Pelvis, Joint::Spine, Joint::Chest],
                            );
                    score += trial.copy_rest(&reading, &start);
                    if best.as_ref().is_none_or(|(b, _)| score < *b) {
                        best = Some((score, trial));
                    }
                }
            }
        }
        let Some((_, posed)) = best else {
            return Err(MarksError::TooFew);
        };
        *self = posed;
        self.stand_on_floor(marks);
        let posed = self.posed();
        let hips = picture_point(marks, End::Hips).ok_or(MarksError::TooFew)?;
        Ok(Fit {
            picture: hips,
            figure: figure_point(&posed, head, End::Hips),
            scale: reading.scale,
        })
    }

    /// The neck and the limbs, each the best of the ways its lines may lean, onto a figure
    /// whose torso is posed; how far they are from their lines all told.
    fn copy_rest(&mut self, reading: &Reading, start: &Pose) -> f64 {
        let head = self.build().head;
        let mut total = 0.0;
        let mut best_of = |figure: &mut Mannequin, trials: Vec<(Mannequin, f64)>| {
            if let Some((chosen, score)) = trials.into_iter().min_by(|a, b| a.1.total_cmp(&b.1)) {
                *figure = chosen;
                total += score;
            }
        };
        let trials: Vec<(Mannequin, f64)> = reading
            .ways(&NECK)
            .into_iter()
            .filter_map(|way| {
                let want = reading.lift(&NECK, way)?;
                let mut trial = *self;
                trial.copy_neck(want);
                let score = miss(&trial.posed(), head, &[(NECK, want)])
                    + NEAREST * moved(&trial.pose, start, &[Joint::Neck, Joint::Head]);
                Some((trial, score))
            })
            .collect();
        best_of(self, trials);
        for limb in LIMBS {
            let [root, hinge, end] = limb;
            let (upper, lower) = (line(root, hinge), line(hinge, end));
            let lower_ways = reading.ways(&lower);
            let lower_ways: Vec<Option<f64>> = if lower_ways.is_empty() {
                vec![None]
            } else {
                lower_ways.into_iter().map(Some).collect()
            };
            let mut trials = Vec::new();
            for up in reading.ways(&upper) {
                for down in &lower_ways {
                    let Some(u) = reading.lift(&upper, up) else {
                        continue;
                    };
                    let l = down.and_then(|d| reading.lift(&lower, d));
                    let mut trial = *self;
                    trial.copy_limb(limb, u, l);
                    let mut wanted = vec![(upper, u)];
                    wanted.extend(l.map(|l| (lower, l)));
                    let score = miss(&trial.posed(), head, &wanted)
                        + NEAREST * moved(&trial.pose, start, &limb);
                    trials.push((trial, score));
                }
            }
            best_of(self, trials);
        }
        total
    }

    /// Lowers or raises the figure so its lowest marked foot rests on the floor.
    fn stand_on_floor(&mut self, marks: &Marks) {
        let posed = self.posed();
        let lowest = [Joint::LeftAnkle, Joint::RightAnkle]
            .into_iter()
            .filter(|j| marks.get(*j).is_some())
            .map(|j| posed.at(j).y)
            .fold(f64::MAX, f64::min);
        if lowest < f64::MAX {
            self.pose
                .set_lift(Vec3::new(0.0, ANKLE_HEIGHT - lowest, 0.0));
        }
    }
}
