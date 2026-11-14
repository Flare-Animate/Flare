//! From the network's seventeen points to the mannequin's marks: the body's joints as they are,
//! the middle of the head from the face, and the neck between the collarbones, a little above
//! the shoulders toward the head. How sure the look was, and where to look again, closer.

use flare_mannequin::{Joint, Marks};

use crate::Found;
use crate::look::{Point, Points, Window};

/// A point the network is less sure of than this is not marked.
pub const MIN_SCORE: f64 = 0.3;
/// A look less sure than this of the body on the whole found no one.
const MIN_CONFIDENCE: f64 = 0.3;
/// How far the neck is from the shoulders' middle toward the middle of the head: the
/// mannequin's own proportion.
const NECK_SHARE: f64 = 0.18;
/// How much wider than the points seen the closer look is, on each side all told.
const MARGIN: f64 = 1.5;
/// How many torsos (shoulders to hips) of room the closer look gives round the hips: a body
/// is about three and a half, with room for raised arms.
const BODY_IN_TORSOS: f64 = 4.2;

const FACE: [usize; 5] = [0, 1, 2, 3, 4];
const LEFT_SHOULDER: usize = 5;
const RIGHT_SHOULDER: usize = 6;
const LEFT_HIP: usize = 11;
const RIGHT_HIP: usize = 12;
/// The body's points and the joints they mark.
const BODY: [(usize, Joint); 12] = [
    (5, Joint::LeftShoulder),
    (6, Joint::RightShoulder),
    (7, Joint::LeftElbow),
    (8, Joint::RightElbow),
    (9, Joint::LeftWrist),
    (10, Joint::RightWrist),
    (11, Joint::LeftHip),
    (12, Joint::RightHip),
    (13, Joint::LeftKnee),
    (14, Joint::RightKnee),
    (15, Joint::LeftAnkle),
    (16, Joint::RightAnkle),
];

fn seen(p: &Point) -> bool {
    p.score >= MIN_SCORE
}

/// How sure a look was of the body, the average of its twelve points.
pub(crate) fn confidence(points: &Points) -> f64 {
    BODY.iter().map(|(i, _)| points[*i].score).sum::<f64>() / BODY.len() as f64
}

/// A square round the points that were seen, with room about them, for a closer look; none
/// when too little was seen to tell where the person is. The points of a far look may bunch
/// up, so the square also takes in a body's height round the hips, from how long the torso
/// is (a standing person is about three and a half torsos tall, the hips at their middle).
pub(crate) fn round_person(points: &Points, [w, h]: [u32; 2]) -> Option<Window> {
    let found: Vec<[f64; 2]> = points.iter().filter(|p| seen(p)).map(|p| p.at).collect();
    if found.len() < 4 || confidence(points) < MIN_CONFIDENCE / 2.0 {
        return None;
    }
    let (mut low, mut high) = ([f64::MAX; 2], [f64::MIN; 2]);
    for at in &found {
        for k in 0..2 {
            low[k] = low[k].min(at[k]);
            high[k] = high[k].max(at[k]);
        }
    }
    // The points are joints, inside the outline: the margin takes in the rest of the body.
    let center = middle(low, high);
    let mut half = (high[0] - low[0]).max(high[1] - low[1]) * MARGIN / 2.0;
    let (mut low, mut high) = (
        [center[0] - half, center[1] - half],
        [center[0] + half, center[1] + half],
    );
    let torso = [LEFT_SHOULDER, RIGHT_SHOULDER, LEFT_HIP, RIGHT_HIP]
        .iter()
        .all(|i| seen(&points[*i]));
    if torso {
        let shoulders = middle(points[LEFT_SHOULDER].at, points[RIGHT_SHOULDER].at);
        let hips = middle(points[LEFT_HIP].at, points[RIGHT_HIP].at);
        half = (shoulders[0] - hips[0]).hypot(shoulders[1] - hips[1]) * BODY_IN_TORSOS / 2.0;
        for k in 0..2 {
            low[k] = low[k].min(hips[k] - half);
            high[k] = high[k].max(hips[k] + half);
        }
    }
    let longest = f64::from(w.max(h));
    let side = (high[0] - low[0])
        .max(high[1] - low[1])
        .clamp(32.0, longest * 1.2);
    Some(Window {
        left: (low[0] + high[0] - side) / 2.0,
        top: (low[1] + high[1] - side) / 2.0,
        side,
    })
}

fn middle(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}

/// The marks of a look, each point multiplied by `scale` into the picture's units; none when
/// the shoulders and hips were not all seen or the look was unsure of the body.
pub(crate) fn to_marks(points: &Points, scale: [f64; 2]) -> Option<Found> {
    let needed = [LEFT_SHOULDER, RIGHT_SHOULDER, LEFT_HIP, RIGHT_HIP];
    let confidence = confidence(points);
    if confidence < MIN_CONFIDENCE || !needed.iter().all(|i| seen(&points[*i])) {
        return None;
    }
    let to = |at: [f64; 2]| [at[0] * scale[0], at[1] * scale[1]];
    let mut marks = Marks::default();
    for (i, joint) in BODY {
        if seen(&points[i]) {
            marks.set(joint, Some(to(points[i].at)));
        }
    }
    let face: Vec<[f64; 2]> = FACE
        .iter()
        .map(|i| points[*i])
        .filter(seen)
        .map(|p| p.at)
        .collect();
    let shoulders = middle(points[LEFT_SHOULDER].at, points[RIGHT_SHOULDER].at);
    let head = (!face.is_empty()).then(|| {
        let n = face.len() as f64;
        [
            face.iter().map(|p| p[0]).sum::<f64>() / n,
            face.iter().map(|p| p[1]).sum::<f64>() / n,
        ]
    });
    let neck = head.map_or(shoulders, |head| {
        [
            shoulders[0] + (head[0] - shoulders[0]) * NECK_SHARE,
            shoulders[1] + (head[1] - shoulders[1]) * NECK_SHARE,
        ]
    });
    marks.set(Joint::Neck, Some(to(neck)));
    if let Some(head) = head {
        marks.set(Joint::Head, Some(to(head)));
    }
    Some(Found { marks, confidence })
}
