//! The body as one smooth surface: tubes of cross-sections (rings) laid along the skeleton at
//! rest, joined into triangles, each point moved by up to two joints in shares (linear blend
//! skinning), so a bent knee or a raised arm stays one surface instead of parts that come apart.
//! Each point also carries where it is on the guide lines drawn over the body.

use crate::kinematics::Posed;
use crate::math::{Rot3, Vec3};
use crate::skeleton::{JOINT_COUNT, Joint};

/// One or two joints and the share of the point each moves; the shares add to one.
pub(crate) type Bones = [(Joint, f32); 2];

pub(crate) fn only(joint: Joint) -> Bones {
    [(joint, 1.0), (joint, 0.0)]
}

/// `from` giving way to `to` as `t` runs from 0 to 1, smoothly.
pub(crate) fn blend(from: Joint, to: Joint, t: f64) -> Bones {
    let t = t.clamp(0.0, 1.0);
    let t = (t * t * (3.0 - 2.0 * t)) as f32;
    [(from, 1.0 - t), (to, t)]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Vertex {
    pub rest: Vec3,
    pub normal: Vec3,
    pub bones: Bones,
    /// Where the point is on the guide lines: round the tube, along it, and round it counted in
    /// center lines; a line is drawn where one is a whole number. A half keeps a surface clear.
    pub guide: [f32; 3],
}

/// The body's surface at rest.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Mesh {
    pub vertices: Vec<Vertex>,
    /// Counterclockwise seen from outside.
    pub triangles: Vec<[u32; 3]>,
    /// The joint a press on each triangle picks.
    pub owners: Vec<Joint>,
}

/// One cross-section: its middle, how far it reaches each way along the tube's two crosswise
/// directions (`u` then `w`, plus then minus), the joints that move it, and where it is along
/// the guide lines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ring {
    pub center: Vec3,
    pub u: (f64, f64),
    pub w: (f64, f64),
    pub bones: Bones,
    pub along: f64,
}

/// How a tube is laid out and lined.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Tube {
    pub u: Vec3,
    pub w: Vec3,
    /// Points round a ring.
    pub around: usize,
    /// Guide lines round the tube, the first down the middle of its `w` side; none at zero.
    pub lines: usize,
    /// Whether the lines along it are drawn.
    pub rings_lined: bool,
    /// Above 2, a cross-section squarer than an ellipse, as a ribcage is.
    pub squareness: f64,
}

fn superellipse(c: f64, n: f64) -> f64 {
    c.signum() * c.abs().powf(2.0 / n)
}

impl Mesh {
    /// Adds a tube through `rings` (two at least); a ring with no size closes an end.
    pub fn tube(&mut self, tube: Tube, rings: &[Ring]) {
        if rings.len() < 2 || tube.around < 3 {
            return;
        }
        let base = self.vertices.len();
        let m = tube.around;
        let cols = m + 1;
        for ring in rings {
            for j in 0..cols {
                let theta =
                    std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * j as f64 / m as f64;
                let (s, c) = theta.sin_cos();
                let pu =
                    superellipse(c, tube.squareness) * if c >= 0.0 { ring.u.0 } else { ring.u.1 };
                let pw =
                    superellipse(s, tube.squareness) * if s >= 0.0 { ring.w.0 } else { ring.w.1 };
                let round = if tube.lines == 0 {
                    0.5
                } else {
                    (j * tube.lines) as f32 / m as f32
                };
                let centers = if tube.lines < 2 {
                    0.5
                } else {
                    round / (tube.lines / 2) as f32
                };
                let along = if tube.rings_lined {
                    ring.along as f32
                } else {
                    0.5
                };
                self.vertices.push(Vertex {
                    rest: ring.center + tube.u.scale(pu) + tube.w.scale(pw),
                    normal: Vec3::ZERO,
                    bones: ring.bones,
                    guide: [round, along, centers],
                });
            }
        }
        // Wind every triangle so it faces out, judged on the widest ring.
        let widest = (0..rings.len() - 1)
            .max_by(|a, b| {
                let size = |r: &Ring| r.u.0 + r.u.1 + r.w.0 + r.w.1;
                size(&rings[*a]).total_cmp(&size(&rings[*b]))
            })
            .unwrap_or(0);
        let at = |i: usize, j: usize| base + i * cols + j;
        let p = |k: usize| self.vertices[k].rest;
        let (a, b, d) = (at(widest, 0), at(widest, 1), at(widest + 1, 0));
        let face = (p(b) - p(a)).cross(p(d) - p(a));
        let flip = face.dot(p(a) - rings[widest].center) < 0.0;
        let first = self.triangles.len();
        for i in 0..rings.len() - 1 {
            for j in 0..m {
                let (a, b, c, d) = (at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j));
                for t in [[a, b, c], [a, c, d]] {
                    let t = t.map(|k| k as u32);
                    self.triangles
                        .push(if flip { [t[0], t[2], t[1]] } else { t });
                }
            }
        }
        self.smooth(base, first, rings.len(), cols);
    }

    /// The tube's normals: each point's faces, weighted by their size, the seam's two columns
    /// and a closed end's points taken together.
    fn smooth(&mut self, base: usize, first: usize, ring_count: usize, cols: usize) {
        for t in first..self.triangles.len() {
            let [a, b, c] = self.triangles[t].map(|k| k as usize);
            let (pa, pb, pc) = (
                self.vertices[a].rest,
                self.vertices[b].rest,
                self.vertices[c].rest,
            );
            let face = (pb - pa).cross(pc - pa);
            for k in [a, b, c] {
                self.vertices[k].normal = self.vertices[k].normal + face;
            }
            let owner = self.vertices[a].bones;
            self.owners.push(if owner[0].1 >= owner[1].1 {
                owner[0].0
            } else {
                owner[1].0
            });
        }
        for i in 0..ring_count {
            let row = base + i * cols;
            let seam = self.vertices[row].normal + self.vertices[row + cols - 1].normal;
            self.vertices[row].normal = seam;
            self.vertices[row + cols - 1].normal = seam;
            let points = &self.vertices[row..row + cols];
            let closed = points
                .iter()
                .all(|v| (v.rest - points[0].rest).length() < 1e-9);
            if closed {
                let sum = points.iter().fold(Vec3::ZERO, |s, v| s + v.normal);
                for v in &mut self.vertices[row..row + cols] {
                    v.normal = sum;
                }
            }
        }
        for v in &mut self.vertices[base..] {
            v.normal = v.normal.normalized().unwrap_or(Vec3::new(0.0, 0.0, 1.0));
        }
    }
}

/// `share` of a joint's turn. The cap of an arm or a leg, shared with the chest or the hips,
/// takes its share of the swing but all of the twist: the bone turns in its socket and the
/// round cap turns with it, where a share of the twist would wind the cap's rings into a fold.
fn part_turn(joint: Joint, turn: Rot3, share: f64) -> Rot3 {
    use Joint::*;
    match joint {
        LeftShoulder | RightShoulder | LeftHip | RightHip => {
            let angles = turn.to_angles();
            let swing = Rot3::about_z(angles.side) * Rot3::about_x(angles.bend);
            swing.partial(share) * Rot3::about_y(angles.twist)
        }
        _ => turn.partial(share),
    }
}

/// The posed surface: every point and the way it faces.
pub(crate) struct Skinned {
    pub points: Vec<Vec3>,
    pub normals: Vec<Vec3>,
}

/// Moves the surface from `rest` to `posed`: each point by its joints' turns, in shares.
///
/// A point shared by a joint and the joint it hangs from is turned that share of the way about
/// the joint, along the arc, instead of set between the two places it would go (linear blend
/// skinning): averaged places cut the corner, so a shoulder raised a quarter turn or a knee
/// bent far would fold flat where the arc keeps the surface round.
pub(crate) fn skin(mesh: &Mesh, rest: &Posed, posed: &Posed) -> Skinned {
    // At rest every frame is square to the figure, so a joint's move is its turn about the
    // place it rested and then the step to where it is.
    let moves: [(Rot3, Vec3); JOINT_COUNT] = Joint::ALL.map(|j| {
        let turn = posed.frame(j);
        (turn, posed.at(j) - turn.apply(rest.at(j)))
    });
    // Each joint's own turn, within the frame of the joint it hangs from.
    let local: [Rot3; JOINT_COUNT] =
        Joint::ALL.map(|j| posed.parent_frame(j).transpose() * posed.frame(j));
    let mut points = Vec::with_capacity(mesh.vertices.len());
    let mut normals = Vec::with_capacity(mesh.vertices.len());
    // A ring's points share their joints, so the part turn is worked out once a ring.
    let mut part: Option<(Bones, Rot3)> = None;
    for v in &mesh.vertices {
        let [(from, from_share), (to, share)] = v.bones;
        let arc = share > 0.0 && from_share > 0.0 && to.parent() == Some(from);
        let (p, n) = if arc {
            let turn = match part {
                Some((bones, turn)) if bones == v.bones => turn,
                _ => {
                    let turn = part_turn(to, local[to.index()], f64::from(share));
                    part = Some((v.bones, turn));
                    turn
                }
            };
            let pivot = rest.at(to);
            let (frame, step) = moves[from.index()];
            let p = pivot + turn.apply(v.rest - pivot);
            (frame.apply(p) + step, frame.apply(turn.apply(v.normal)))
        } else {
            let (mut p, mut n) = (Vec3::ZERO, Vec3::ZERO);
            for (joint, share) in v.bones {
                if share > 0.0 {
                    let (turn, step) = moves[joint.index()];
                    let share = f64::from(share);
                    p = p + (turn.apply(v.rest) + step).scale(share);
                    n = n + turn.apply(v.normal).scale(share);
                }
            }
            (p, n)
        };
        points.push(p);
        normals.push(n.normalized().unwrap_or(v.normal));
    }
    Skinned { points, normals }
}
