//! Shape armatures: bones drawn *inside* one shape (Animate's shape bones).
//! The shape's points follow the bones (linear-blend skinning); the Bind tool
//! pins points to a particular bone. `Shape::path` stays the rest shape; the
//! posed shape is computed when drawing.

use crate::geom::{pt, Matrix, Path, Pt, Seg};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RigBone {
    pub parent: Option<usize>,
    /// Rest position, shape coordinates.
    pub head: Pt,
    pub tail: Pt,
    /// Pose: rotation about the head relative to rest (radians).
    #[serde(default)]
    pub angle: f32,
    /// Joint: Rotation ▸ Enable (off: the bone keeps its angle while posing).
    #[serde(default = "yes", skip_serializing_if = "is_true")]
    pub rotate: bool,
    /// Joint: Rotation ▸ Constrain: allowed `angle` range in degrees (min ≤ 0 ≤ max).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<(f32, f32)>,
    /// Pinned tail: while posing, the bone's tail stays here (posed shape coordinates).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<Pt>,
}

fn yes() -> bool {
    true
}
fn is_true(b: &bool) -> bool {
    *b
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ShapeRig {
    pub bones: Vec<RigBone>,
    /// Bind tool overrides: point index (in path order) → bone. Points without
    /// one are weighted by distance to the nearest bones.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bind: Vec<(usize, usize)>,
}

/// Every point of a path in order (anchors and control points).
pub fn path_points(p: &Path) -> Vec<Pt> {
    let mut v = vec![];
    for s in &p.segs {
        match *s {
            Seg::M(a) | Seg::L(a) => v.push(a),
            Seg::Q(a, b) => v.extend([a, b]),
            Seg::C(a, b, c) => v.extend([a, b, c]),
            Seg::Z => {}
        }
    }
    v
}
fn with_points(p: &Path, pts: &[Pt]) -> Path {
    let mut i = 0;
    let mut next = || {
        let x = pts[i];
        i += 1;
        x
    };
    Path {
        segs: p
            .segs
            .iter()
            .map(|s| match s {
                Seg::M(_) => Seg::M(next()),
                Seg::L(_) => Seg::L(next()),
                Seg::Q(..) => Seg::Q(next(), next()),
                Seg::C(..) => Seg::C(next(), next(), next()),
                Seg::Z => Seg::Z,
            })
            .collect(),
    }
}
/// Indices of anchor points (end points of segments) in `path_points` order.
pub fn anchor_indices(p: &Path) -> Vec<usize> {
    let mut v = vec![];
    let mut i = 0;
    for s in &p.segs {
        match s {
            Seg::M(_) | Seg::L(_) => {
                v.push(i);
                i += 1;
            }
            Seg::Q(..) => {
                v.push(i + 1);
                i += 2;
            }
            Seg::C(..) => {
                v.push(i + 2);
                i += 3;
            }
            Seg::Z => {}
        }
    }
    v
}

/// Give a shape enough points to bend: curves are flattened and long edges
/// get extra points every `step` units (done when the first bone is added).
pub fn densify(path: &Path, step: f32) -> Path {
    let mut segs = vec![];
    for (pts, closed) in path.flatten(16) {
        if pts.is_empty() {
            continue;
        }
        let mut ring = pts.clone();
        if closed {
            ring.push(pts[0]);
        }
        segs.push(Seg::M(ring[0]));
        let last = ring.len() - 2;
        for (i, w) in ring.windows(2).enumerate() {
            let n = (w[0].dist(w[1]) / step).ceil().max(1.0) as usize;
            // on a closed ring the final point is the start again: Z joins it
            let upto = if closed && i == last { n - 1 } else { n };
            for k in 1..=upto {
                segs.push(Seg::L(w[0].lerp(w[1], k as f32 / n as f32)));
            }
        }
        if closed {
            segs.push(Seg::Z);
        }
    }
    Path { segs }
}

fn seg_dist(x: Pt, a: Pt, b: Pt) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 < 1e-9 { 0.0 } else { (((x.x - a.x) * dx + (x.y - a.y) * dy) / l2).clamp(0.0, 1.0) };
    pt(a.x + dx * t, a.y + dy * t).dist(x)
}

fn rot_about(c: Pt, a: f32) -> Matrix {
    Matrix::translate(c.x, c.y).concat(&Matrix::rotate(a)).concat(&Matrix::translate(-c.x, -c.y))
}

impl ShapeRig {
    /// Posed transform of each bone (shape coordinates, rest → pose).
    pub fn transforms(&self) -> Vec<Matrix> {
        let mut out: Vec<Option<Matrix>> = vec![None; self.bones.len()];
        fn get(r: &ShapeRig, i: usize, out: &mut Vec<Option<Matrix>>, depth: usize) -> Matrix {
            if let Some(m) = out[i] {
                return m;
            }
            let b = r.bones[i];
            let parent = match b.parent.filter(|p| *p < r.bones.len() && depth < 64) {
                Some(p) => get(r, p, out, depth + 1),
                None => Matrix::IDENTITY,
            };
            let m = parent.concat(&rot_about(b.head, b.angle));
            out[i] = Some(m);
            m
        }
        (0..self.bones.len()).map(|i| get(self, i, &mut out, 0)).collect()
    }
    /// Posed head/tail of every bone.
    pub fn posed(&self) -> Vec<(Pt, Pt)> {
        let t = self.transforms();
        self.bones.iter().zip(&t).map(|(b, m)| (m.apply(b.head), m.apply(b.tail))).collect()
    }

    /// Weights of a rest point: the Bind tool's choice, else the two nearest bones.
    fn weights(&self, idx: usize, x: Pt) -> Vec<(usize, f32)> {
        if let Some(&(_, b)) = self.bind.iter().find(|(i, b)| *i == idx && *b < self.bones.len()) {
            return vec![(b, 1.0)];
        }
        let mut d: Vec<(usize, f32)> = self.bones.iter().enumerate().map(|(i, b)| (i, seg_dist(x, b.head, b.tail).max(0.5))).collect();
        d.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        d.truncate(2);
        let ws: Vec<(usize, f32)> = d.iter().map(|(i, dd)| (*i, 1.0 / (dd * dd))).collect();
        let sum: f32 = ws.iter().map(|w| w.1).sum();
        ws.into_iter().map(|(i, w)| (i, w / sum)).collect()
    }

    /// The bone a point follows most (for showing bindings).
    pub fn main_bone(&self, idx: usize, x: Pt) -> Option<usize> {
        self.weights(idx, x).into_iter().max_by(|a, b| a.1.partial_cmp(&b.1).unwrap()).map(|w| w.0)
    }

    /// Pose `rest` by the bones.
    pub fn deform(&self, rest: &Path) -> Path {
        if self.bones.is_empty() || self.bones.iter().all(|b| b.angle == 0.0) {
            return rest.clone();
        }
        let t = self.transforms();
        let pts: Vec<Pt> = path_points(rest)
            .into_iter()
            .enumerate()
            .map(|(i, x)| {
                let mut acc = pt(0.0, 0.0);
                for (b, w) in self.weights(i, x) {
                    let q = t[b].apply(x);
                    acc = pt(acc.x + q.x * w, acc.y + q.y * w);
                }
                acc
            })
            .collect();
        with_points(rest, &pts)
    }

    /// Add a bone from `head` to `tail` (rest coordinates); it chains onto a
    /// bone whose tail is within `snap` of `head`.
    pub fn add_bone(&mut self, head: Pt, tail: Pt, snap: f32) -> usize {
        let posed = self.posed();
        let parent = posed.iter().position(|(_, t)| t.dist(head) <= snap);
        // new bones are added in rest space: undo the parent's pose
        let (h, t) = match parent {
            Some(p) => {
                let inv = self.transforms()[p].invert().unwrap_or(Matrix::IDENTITY);
                (self.bones[p].tail, inv.apply(tail))
            }
            None => (head, tail),
        };
        self.bones.push(RigBone { parent, head: h, tail: t, angle: 0.0, rotate: true, limit: None, pin: None });
        self.bones.len() - 1
    }

    /// Inverse kinematics: move the tail of `bone` toward `target` (posed
    /// coordinates), turning the armature's joints (damped least squares;
    /// pinned tails stay where they were pinned, limits hold).
    pub fn ik(&mut self, bone: usize, target: Pt) {
        if bone >= self.bones.len() {
            return;
        }
        let mut eff = vec![crate::ik::Effector { owner: bone, local: self.bones[bone].tail, target, weight: 1.0 }];
        for (i, b) in self.bones.iter().enumerate() {
            if let Some(p) = b.pin {
                eff.push(crate::ik::Effector { owner: i, local: b.tail, target: p, weight: 4.0 });
            }
        }
        crate::ik::solve_dls(self, &eff, 80);
    }

    /// Bone `i`'s ancestors, nearest first.
    fn ancestors(&self, i: usize) -> Vec<usize> {
        let mut v = vec![];
        let mut cur = self.bones.get(i).and_then(|b| b.parent);
        while let Some(p) = cur.filter(|p| *p < self.bones.len() && v.len() < 64) {
            v.push(p);
            cur = self.bones[p].parent;
        }
        v
    }

    /// Bind tool: pin point `idx` to `bone` (None = back to automatic).
    pub fn set_bind(&mut self, idx: usize, bone: Option<usize>) {
        self.bind.retain(|(i, _)| *i != idx);
        if let Some(b) = bone {
            self.bind.push((idx, b));
        }
    }

    pub fn lerp(a: &ShapeRig, b: &ShapeRig, t: f32) -> ShapeRig {
        let mut out = a.clone();
        if a.bones.len() == b.bones.len() {
            for (o, (x, y)) in out.bones.iter_mut().zip(a.bones.iter().zip(&b.bones)) {
                o.angle = x.angle + (y.angle - x.angle) * t;
            }
        }
        out
    }
}

impl crate::ik::Kinematic for ShapeRig {
    fn joints(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.bones.len()).collect();
        v.sort_by_key(|&i| self.ancestors(i).len());
        v
    }
    fn pivot(&self, j: usize) -> Pt {
        self.posed()[j].0
    }
    fn moves(&self, j: usize, owner: usize) -> bool {
        owner == j || self.ancestors(owner).contains(&j)
    }
    fn world(&self, owner: usize, local: Pt) -> Pt {
        self.transforms()[owner].apply(local)
    }
    fn turn(&mut self, j: usize, d: f32) {
        let b = &mut self.bones[j];
        if !b.rotate {
            return;
        }
        let mut a = b.angle + d;
        if let Some((lo, hi)) = b.limit {
            a = a.clamp(lo.min(0.0).to_radians(), hi.max(0.0).to_radians());
        }
        b.angle = a;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bones_bend_a_shape_and_bind_pins_points() {
        // a long bar with two bones along it
        let bar = Path::rect(0.0, -10.0, 200.0, 20.0);
        let mut r = ShapeRig::default();
        r.add_bone(pt(0.0, 0.0), pt(100.0, 0.0), 2.0);
        let b2 = r.add_bone(pt(100.0, 0.0), pt(200.0, 0.0), 2.0);
        assert_eq!(r.bones[b2].parent, Some(0));
        // IK: pull the end up
        r.ik(b2, pt(150.0, 120.0));
        assert!(r.posed()[b2].1.dist(pt(150.0, 120.0)) < 1.0, "{:?}", r.posed()[b2]);
        let posed = r.deform(&bar);
        let pts = path_points(&posed);
        // the far corners moved up a lot, the near corners stayed
        assert!(pts.iter().any(|p| p.y > 80.0));
        assert!(pts.iter().any(|p| p.dist(pt(0.0, 0.0)) < 11.0), "the root end stays at the root");
        // Bind: pin the far top-right corner to the root bone
        let idx = path_points(&bar).iter().position(|p| p.dist(pt(200.0, -10.0)) < 0.01).unwrap();
        let before = path_points(&posed)[idx];
        r.set_bind(idx, Some(0));
        let after = path_points(&r.deform(&bar))[idx];
        assert!(before.dist(after) > 10.0, "binding changed what the point follows");
        // tween: half-way pose
        let rest = ShapeRig { bones: r.bones.iter().map(|b| RigBone { angle: 0.0, ..*b }).collect(), bind: vec![] };
        let mid = ShapeRig::lerp(&rest, &r, 0.5);
        assert!((mid.bones[0].angle - r.bones[0].angle / 2.0).abs() < 1e-5);
    }
}
