//! The Segment type on a line: where along it the edges of what crosses it, and the line itself,
//! meet it, which stretches of it the eraser touches, and the pieces left between.

use flare_core::DocumentPoint;
use flare_document::{MAX_SEGMENTS, PathSegment};
use glamx::DVec2;

use super::{MAX_SELF_CROSSING_POINTS, SAME};
use crate::common::cut::Contour;

/// One line of a shape: its points in order, and whether it closes back to where it began (its
/// last point is then its first again).
pub(super) struct Line {
    pub(super) points: Vec<DVec2>,
    pub(super) closed: bool,
}

impl Line {
    /// How far along the line each point is.
    fn lengths(&self) -> Vec<f64> {
        let mut at = 0.0;
        let mut out = Vec::with_capacity(self.points.len());
        out.push(0.0);
        for pair in self.points.windows(2) {
            at += pair[0].distance(pair[1]);
            out.push(at);
        }
        out
    }
}

/// The pieces of `lines` left once each stretch `eraser` touches is taken out to the nearest
/// places on either side where the edge of `crossing`, or the line itself, crosses it; `None`
/// when the eraser touches none of them.
pub(super) fn line_parts(
    lines: &[Line],
    crossing: &[Contour],
    eraser: &[Contour],
) -> Option<Vec<Line>> {
    let edges: Vec<(DVec2, DVec2)> = crossing.iter().flat_map(closed_edges).collect();
    let eraser_edges: Vec<(DVec2, DVec2)> = eraser.iter().flat_map(closed_edges).collect();
    let mut any = false;
    let mut out = Vec::new();
    for (k, line) in lines.iter().enumerate() {
        let lengths = line.lengths();
        let total = lengths.last().copied().unwrap_or(0.0);
        let hits = touched(line, &lengths, eraser, &eraser_edges);
        if hits.is_empty() || total <= SAME {
            out.push(Line {
                points: line.points.clone(),
                closed: line.closed,
            });
            continue;
        }
        any = true;
        // The other lines of the same shape cross it too.
        let mut own: Vec<(DVec2, DVec2)> = Vec::new();
        for (j, other) in lines.iter().enumerate() {
            if j != k {
                own.extend(other.points.windows(2).map(|w| (w[0], w[1])));
            }
        }
        let mut cuts = crossings(line, &lengths, &edges);
        cuts.extend(crossings(line, &lengths, &own));
        cuts.extend(self_crossings(line, &lengths));
        cuts.retain(|s| *s > SAME && *s < total - SAME);
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() <= SAME);
        out.extend(kept(line, &lengths, total, &cuts, &hits));
    }
    any.then_some(out)
}

/// The edges of the closed outline `contour`.
fn closed_edges(contour: &Contour) -> Vec<(DVec2, DVec2)> {
    let points: Vec<DVec2> = contour.iter().map(|p| DVec2::new(p[0], p[1])).collect();
    let n = points.len();
    (0..n).map(|i| (points[i], points[(i + 1) % n])).collect()
}

/// Where along their lengths the segments `a` (from `p` to `q`) and `b` meet, as shares of each,
/// both from 0 to 1; `None` when they do not, or run side by side.
fn meet(p: DVec2, q: DVec2, a: DVec2, b: DVec2) -> Option<(f64, f64)> {
    let (r, s) = (q - p, b - a);
    let denominator = r.perp_dot(s);
    if denominator.abs() <= f64::EPSILON * r.length() * s.length() {
        return None;
    }
    let t = (a - p).perp_dot(s) / denominator;
    let u = (a - p).perp_dot(r) / denominator;
    let near = |v: f64| (-1e-9..=1.0 + 1e-9).contains(&v);
    (near(t) && near(u)).then_some((t.clamp(0.0, 1.0), u.clamp(0.0, 1.0)))
}

/// How far along `line` it meets each of `edges`.
fn crossings(line: &Line, lengths: &[f64], edges: &[(DVec2, DVec2)]) -> Vec<f64> {
    let mut out = Vec::new();
    for (i, pair) in line.points.windows(2).enumerate() {
        let (p, q) = (pair[0], pair[1]);
        let (lo, hi) = (p.min(q), p.max(q));
        for &(a, b) in edges {
            if a.max(b).cmplt(lo).any() || a.min(b).cmpgt(hi).any() {
                continue;
            }
            if let Some((t, _)) = meet(p, q, a, b) {
                out.push(lengths[i] + t * (lengths[i + 1] - lengths[i]));
            }
        }
    }
    out
}

/// How far along `line` it crosses itself, at both places on it each time.
fn self_crossings(line: &Line, lengths: &[f64]) -> Vec<f64> {
    let n = line.points.len();
    if n > MAX_SELF_CROSSING_POINTS {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..n.saturating_sub(1) {
        let (p, q) = (line.points[i], line.points[i + 1]);
        // A segment meets the next where they join, which is no crossing; a closed line's last
        // segment joins its first the same way.
        for j in (i + 2)..n.saturating_sub(1) {
            if line.closed && i == 0 && j == n - 2 {
                continue;
            }
            let (a, b) = (line.points[j], line.points[j + 1]);
            if let Some((t, u)) = meet(p, q, a, b) {
                out.push(lengths[i] + t * (lengths[i + 1] - lengths[i]));
                out.push(lengths[j] + u * (lengths[j + 1] - lengths[j]));
            }
        }
    }
    out
}

/// The stretches of `line`, as lengths along it, that lie inside the closed outlines `eraser`
/// (whose edges are `edges`).
fn touched(
    line: &Line,
    lengths: &[f64],
    eraser: &[Contour],
    edges: &[(DVec2, DVec2)],
) -> Vec<(f64, f64)> {
    let (lo, hi) = edges.iter().fold(
        (DVec2::splat(f64::MAX), DVec2::splat(f64::MIN)),
        |(lo, hi), (a, _)| (lo.min(*a), hi.max(*a)),
    );
    let mut out = Vec::new();
    for (i, pair) in line.points.windows(2).enumerate() {
        let (p, q) = (pair[0], pair[1]);
        if p.max(q).cmplt(lo).any() || p.min(q).cmpgt(hi).any() {
            continue;
        }
        let mut at = vec![0.0, 1.0];
        for &(a, b) in edges {
            if let Some((t, _)) = meet(p, q, a, b) {
                at.push(t);
            }
        }
        at.sort_by(f64::total_cmp);
        let (from, span) = (lengths[i], lengths[i + 1] - lengths[i]);
        for w in at.windows(2) {
            if w[1] - w[0] <= SAME {
                continue;
            }
            if inside(p.lerp(q, (w[0] + w[1]) / 2.0), eraser) {
                out.push((from + w[0] * span, from + w[1] * span));
            }
        }
    }
    out
}

/// Whether `point` is inside any of the closed outlines `areas`, by their winding.
fn inside(point: DVec2, areas: &[Contour]) -> bool {
    areas.iter().any(|area| {
        let mut winding = 0i32;
        let n = area.len();
        for i in 0..n {
            let (a, b) = (area[i], area[(i + 1) % n]);
            let (a, b) = (DVec2::new(a[0], a[1]), DVec2::new(b[0], b[1]));
            let side = (b - a).perp_dot(point - a);
            if a.y <= point.y {
                if b.y > point.y && side > 0.0 {
                    winding += 1;
                }
            } else if b.y <= point.y && side < 0.0 {
                winding -= 1;
            }
        }
        winding != 0
    })
}

/// The pieces of `line` between the places `cuts` (sorted lengths along it) that none of `hits`
/// falls on, those that meet joined into one.
fn kept(line: &Line, lengths: &[f64], total: f64, cuts: &[f64], hits: &[(f64, f64)]) -> Vec<Line> {
    // A piece is hit where the eraser covers some of its length; a closed line's last piece
    // runs on past its start, so the hits are looked for there too.
    let hit = |from: f64, to: f64| {
        hits.iter().any(|&(a, b)| {
            [0.0, total, 2.0 * total]
                .iter()
                .any(|shift| a + shift < to && b + shift > from)
        })
    };
    let mut pieces: Vec<(f64, f64)> = Vec::new();
    if line.closed {
        if cuts.is_empty() {
            return if hit(0.0, total) {
                Vec::new()
            } else {
                vec![Line {
                    points: line.points.clone(),
                    closed: true,
                }]
            };
        }
        let mut ring: Vec<(f64, f64)> = cuts.windows(2).map(|w| (w[0], w[1])).collect();
        if let (Some(&last), Some(&first)) = (cuts.last(), cuts.first()) {
            ring.push((last, first + total));
        }
        // Start just after a piece that goes, so the pieces kept on either side of the line's
        // start join up.
        let Some(gone) = ring.iter().position(|&(a, b)| hit(a, b)) else {
            return vec![Line {
                points: line.points.clone(),
                closed: true,
            }];
        };
        let (before, after) = ring.split_at(gone + 1);
        pieces.extend(after.iter().copied());
        pieces.extend(before.iter().map(|&(a, b)| (a + total, b + total)));
    } else {
        let mut at = vec![0.0];
        at.extend_from_slice(cuts);
        at.push(total);
        pieces.extend(at.windows(2).map(|w| (w[0], w[1])));
    }
    let mut runs: Vec<(f64, f64)> = Vec::new();
    let mut open: Option<(f64, f64)> = None;
    for (from, to) in pieces {
        if hit(from, to) {
            runs.extend(open.take());
        } else {
            open = Some(open.map_or((from, to), |(start, _)| (start, to)));
        }
    }
    runs.extend(open);
    runs.into_iter()
        .map(|(from, to)| Line {
            points: stretch(line, lengths, total, from, to),
            closed: false,
        })
        .filter(|piece| piece.points.len() >= 2)
        .collect()
}

/// The points of `line` from `from` to `to` along it; round a closed line, a length past `total`
/// goes on from its start.
fn stretch(line: &Line, lengths: &[f64], total: f64, from: f64, to: f64) -> Vec<DVec2> {
    let (mut from, mut to) = (from, to);
    while from >= total && total > 0.0 {
        from -= total;
        to -= total;
    }
    let at = |s: f64| -> DVec2 {
        let i = lengths
            .partition_point(|l| *l <= s)
            .clamp(1, lengths.len() - 1);
        let (a, b) = (lengths[i - 1], lengths[i]);
        let t = if b > a { (s - a) / (b - a) } else { 0.0 };
        line.points[i - 1].lerp(line.points[i], t.clamp(0.0, 1.0))
    };
    let between = |from: f64, to: f64, out: &mut Vec<DVec2>| {
        out.push(at(from));
        out.extend(
            lengths
                .iter()
                .zip(&line.points)
                .filter(|(l, _)| **l > from && **l < to)
                .map(|(_, p)| *p),
        );
        out.push(at(to));
    };
    let mut out = Vec::new();
    if to <= total {
        between(from, to, &mut out);
    } else {
        between(from, total, &mut out);
        between(0.0, to - total, &mut out);
    }
    out.dedup_by(|a, b| a.distance(*b) <= SAME);
    out
}

/// A path through `lines`, each closed or left open.
pub(super) fn path_of_lines(lines: &[Line]) -> Option<Vec<PathSegment>> {
    let point = |p: DVec2| DocumentPoint::new(p.x, p.y).ok();
    let mut path = Vec::new();
    for line in lines.iter().filter(|l| l.points.len() >= 2) {
        let points = if line.closed {
            &line.points[..line.points.len() - 1]
        } else {
            &line.points[..]
        };
        path.push(PathSegment::MoveTo(point(*points.first()?)?));
        for p in &points[1..] {
            path.push(PathSegment::LineTo(point(*p)?));
        }
        if line.closed {
            path.push(PathSegment::Close);
        }
    }
    (!path.is_empty() && path.len() <= MAX_SEGMENTS).then_some(path)
}
