//! Shapes as edge lists, for comparing a published shape with its original.

use flare_core::{Rgba8, ShapeId};
use flare_document::{PathSegment, Shape, Stroke};
use glamx::DAffine2;
use swf::{FillStyle, ShapeRecord};

use super::*;

pub fn to_affine(m: &swf::Matrix) -> DAffine2 {
    DAffine2::from_cols_array(&[
        m.a.to_f64(),
        m.b.to_f64(),
        m.c.to_f64(),
        m.d.to_f64(),
        m.tx.to_pixels(),
        m.ty.to_pixels(),
    ])
}

// ---- rebuilding shapes from SWF records ------------------------------------------------

pub type P = (i32, i32);

#[derive(Clone, Copy)]
pub struct Seg {
    pub from: P,
    pub ctrl: Option<P>,
    pub to: P,
    pub fill0: u32,
    pub fill1: u32,
    pub line: u32,
}

pub fn segments(shape: &swf::Shape) -> Vec<Seg> {
    let (mut at, mut f0, mut f1, mut line) = ((0, 0), 0, 0, 0);
    let mut out = Vec::new();
    for record in &shape.shape {
        match record {
            ShapeRecord::StyleChange(data) => {
                if let Some(m) = &data.move_to {
                    at = (m.x.get(), m.y.get());
                }
                f0 = data.fill_style_0.unwrap_or(f0);
                f1 = data.fill_style_1.unwrap_or(f1);
                line = data.line_style.unwrap_or(line);
            }
            ShapeRecord::StraightEdge { delta } => {
                let to = (at.0 + delta.dx.get(), at.1 + delta.dy.get());
                out.push(Seg {
                    from: at,
                    ctrl: None,
                    to,
                    fill0: f0,
                    fill1: f1,
                    line,
                });
                at = to;
            }
            ShapeRecord::CurvedEdge {
                control_delta,
                anchor_delta,
            } => {
                let ctrl = (at.0 + control_delta.dx.get(), at.1 + control_delta.dy.get());
                let to = (
                    ctrl.0 + anchor_delta.dx.get(),
                    ctrl.1 + anchor_delta.dy.get(),
                );
                out.push(Seg {
                    from: at,
                    ctrl: Some(ctrl),
                    to,
                    fill0: f0,
                    fill1: f1,
                    line,
                });
                at = to;
            }
        }
    }
    out
}

pub fn reversed(s: &Seg) -> Seg {
    Seg {
        from: s.to,
        to: s.from,
        ..*s
    }
}

/// Links segments end to start into contours (each segment used once).
pub fn chain(segs: &[Seg]) -> Vec<(Vec<Seg>, bool)> {
    let mut used = vec![false; segs.len()];
    let mut out = Vec::new();
    for first in 0..segs.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let mut run = vec![segs[first]];
        let origin = segs[first].from;
        let mut closed = false;
        loop {
            let end = run[run.len() - 1].to;
            if end == origin {
                closed = true;
                break;
            }
            match (0..segs.len()).find(|i| !used[*i] && segs[*i].from == end) {
                Some(i) => {
                    used[i] = true;
                    run.push(segs[i]);
                }
                None => break,
            }
        }
        out.push((run, closed));
    }
    out
}

pub fn px(p: P) -> (f64, f64) {
    (f64::from(p.0) / 20.0, f64::from(p.1) / 20.0)
}

pub fn path_of(contours: &[(Vec<Seg>, bool)], close_always: bool) -> Vec<PathSegment> {
    let mut path = Vec::new();
    for (run, closed) in contours {
        let (x, y) = px(run[0].from);
        path.push(PathSegment::MoveTo(pt(x, y)));
        for s in run {
            let to = px(s.to);
            match s.ctrl {
                None => path.push(PathSegment::LineTo(pt(to.0, to.1))),
                Some(c) => {
                    let (p0, c, p1) = (px(s.from), px(c), to);
                    let third = |a: (f64, f64)| {
                        (a.0 + 2.0 / 3.0 * (c.0 - a.0), a.1 + 2.0 / 3.0 * (c.1 - a.1))
                    };
                    let (c1, c2) = (third(p0), third(p1));
                    path.push(PathSegment::CubicTo(
                        pt(c1.0, c1.1),
                        pt(c2.0, c2.1),
                        pt(to.0, to.1),
                    ));
                }
            }
        }
        if *closed || close_always {
            path.push(PathSegment::Close);
        }
    }
    path
}

/// Flare shapes equivalent to one SWF shape: one per fill style, then one for the strokes.
pub fn rebuild(shape: &swf::Shape) -> Vec<Shape> {
    let segs = segments(shape);
    let mut out = Vec::new();
    for (index, style) in shape.styles.fill_styles.iter().enumerate() {
        let id = u32::try_from(index + 1).expect("small");
        let FillStyle::Color(c) = style else {
            panic!("only solid fills are published")
        };
        let mut edges: Vec<Seg> = segs.iter().filter(|s| s.fill1 == id).copied().collect();
        edges.extend(segs.iter().filter(|s| s.fill0 == id).map(reversed));
        let path = path_of(&chain(&edges), true);
        if !path.is_empty() {
            out.push(
                Shape::new(ShapeId::new(), path, Rgba8::new(c.r, c.g, c.b, c.a)).expect("fill"),
            );
        }
    }
    for (index, style) in shape.styles.line_styles.iter().enumerate() {
        let id = u32::try_from(index + 1).expect("small");
        let FillStyle::Color(c) = style.fill_style() else {
            panic!("only solid strokes are published")
        };
        let edges: Vec<Seg> = segs.iter().filter(|s| s.line == id).copied().collect();
        let path = path_of(&chain(&edges), false);
        if !path.is_empty() {
            let stroke = Stroke::new(style.width().to_pixels(), Rgba8::new(c.r, c.g, c.b, c.a))
                .expect("stroke");
            out.push(
                Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
                    .expect("outline")
                    .with_stroke(stroke),
            );
        }
    }
    out
}

// ---- SWF's own fill semantics ---------------------------------------------------------
//
// Rendering the rebuilt contours with a winding rule treats a global swap of "left" and
// "right" as harmless, but a SWF player does not: a fill style applies to one *side of each
// edge*. These checks use that definition directly, so they catch inverted fills.

pub fn flatten(seg: &Seg) -> Vec<((f64, f64), (f64, f64))> {
    let (a, b) = (px(seg.from), px(seg.to));
    match seg.ctrl {
        None => vec![(a, b)],
        Some(c) => {
            let c = px(c);
            let at = |t: f64| {
                let u = 1.0 - t;
                (
                    u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0,
                    u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1,
                )
            };
            (0..16)
                .map(|i| (at(f64::from(i) / 16.0), at(f64::from(i + 1) / 16.0)))
                .collect()
        }
    }
}

/// The fill style SWF assigns to `p`: that of the nearest edge, on the side `p` lies.
/// `None` when `p` is within `margin` px of an edge, or when its nearest feature is an
/// edge's end point (a corner), where "which side" is not defined by one edge alone.
pub fn swf_fill_at(segs: &[Seg], p: (f64, f64), margin: f64) -> Option<u32> {
    let mut best: Option<(f64, u32, bool)> = None;
    for seg in segs {
        for (a, b) in flatten(seg) {
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let length2 = dx * dx + dy * dy;
            if length2 < 1e-12 {
                continue;
            }
            let along = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / length2;
            let t = along.clamp(0.0, 1.0);
            let nearest = (a.0 + t * dx, a.1 + t * dy);
            let distance = (p.0 - nearest.0).hypot(p.1 - nearest.1);
            // With y pointing down, a positive cross product puts `p` on the right of travel.
            let right = dx * (p.1 - a.1) - dy * (p.0 - a.0) > 0.0;
            let style = if right { seg.fill1 } else { seg.fill0 };
            let interior = along > 0.0 && along < 1.0;
            if best.is_none_or(|(d, _, _)| distance < d) {
                best = Some((distance, style, interior));
            }
        }
    }
    best.filter(|(d, _, interior)| *d > margin && *interior)
        .map(|(_, style, _)| style)
}

pub fn original_contains(shape: &Shape, p: (f64, f64)) -> bool {
    // Non-zero winding over each contour of the original path (straight edges only here).
    let mut polys: Vec<Vec<(f64, f64)>> = Vec::new();
    for segment in shape.path() {
        match segment {
            PathSegment::MoveTo(q) => polys.push(vec![(q.as_vec().x, q.as_vec().y)]),
            PathSegment::LineTo(q) => polys
                .last_mut()
                .expect("moveto")
                .push((q.as_vec().x, q.as_vec().y)),
            PathSegment::CubicTo(..) => panic!("straight edges only"),
            PathSegment::Close => {}
        }
    }
    let mut winding = 0;
    for poly in &polys {
        for i in 0..poly.len() {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let cross = (b.0 - a.0) * (p.1 - a.1) - (p.0 - a.0) * (b.1 - a.1);
            if a.1 <= p.1 && b.1 > p.1 && cross > 0.0 {
                winding += 1;
            } else if a.1 > p.1 && b.1 <= p.1 && cross < 0.0 {
                winding -= 1;
            }
        }
    }
    winding != 0
}
