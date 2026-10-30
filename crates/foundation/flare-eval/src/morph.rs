//! Shape tweens: one shape becoming another, and the easing both kinds of tween share.
//!
//! The two outlines are matched contour by contour: every segment becomes a cubic, the shorter
//! contour is split until both have as many segments as the other, a closed contour is turned
//! (and flipped, if it winds the other way) to the starting point that moves its points least, and
//! then points, colors and widths are blended. Splitting leaves a curve exactly where it was, so
//! the picture at the first frame and at the last is the shape itself. A contour that has no
//! partner shrinks to its own middle, or grows from it. Animate's shape hints, which let a person
//! choose which points match, are not supported.

use flare_core::{DocumentPoint, Rgba8};
use flare_document::{Gradient, MAX_EASE, PathSegment, Shape, Stroke};
use glamx::DVec2;

/// The most contours and the most segments in a contour a morph will take, so a huge shape cannot
/// make an unbounded amount of work.
const MAX_CONTOURS: usize = 256;
const MAX_SEGMENTS: usize = 4096;
/// Above this many segments a closed contour keeps its starting point instead of searching for a
/// better one (the search compares every pair).
const MAX_TURN_SEARCH: usize = 512;

/// A path through `t` as the tween's easing bends it: the fraction `t` of the way in time becomes
/// this fraction of the way in change. `ease` is Animate's Ease, from `-MAX_EASE` (slow at the
/// start) to `MAX_EASE` (slow at the end); 0 is even speed. The ends stay put and the ease at
/// either extreme is a quadratic curve; between, it blends with a straight line.
pub fn eased(t: f64, ease: i8) -> f64 {
    // A number that is not one is the start; `clamp` would pass it through.
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) };
    let k = f64::from(ease.clamp(-MAX_EASE, MAX_EASE)) / f64::from(MAX_EASE);
    if k >= 0.0 {
        (1.0 - k) * t + k * (1.0 - (1.0 - t) * (1.0 - t))
    } else {
        (1.0 + k) * t - k * t * t
    }
}

#[derive(Clone)]
struct Contour {
    start: DVec2,
    /// Cubic pieces: two control points and the end.
    pieces: Vec<[DVec2; 3]>,
    closed: bool,
}

fn contours(path: &[PathSegment]) -> Option<Vec<Contour>> {
    let mut out: Vec<Contour> = Vec::new();
    let mut at = DVec2::ZERO;
    for segment in path {
        match segment {
            PathSegment::MoveTo(p) => {
                at = p.as_vec();
                out.push(Contour {
                    start: at,
                    pieces: Vec::new(),
                    closed: false,
                });
            }
            PathSegment::LineTo(p) => {
                let to = p.as_vec();
                out.last_mut()?.pieces.push([
                    at + (to - at) / 3.0,
                    at + (to - at) * (2.0 / 3.0),
                    to,
                ]);
                at = to;
            }
            PathSegment::CubicTo(a, b, c) => {
                out.last_mut()?
                    .pieces
                    .push([a.as_vec(), b.as_vec(), c.as_vec()]);
                at = c.as_vec();
            }
            PathSegment::Close => {
                let contour = out.last_mut()?;
                let start = contour.start;
                if at != start {
                    contour.pieces.push([
                        at + (start - at) / 3.0,
                        at + (start - at) * (2.0 / 3.0),
                        start,
                    ]);
                }
                contour.closed = true;
                at = start;
            }
        }
    }
    out.retain(|c| !c.pieces.is_empty());
    (out.len() <= MAX_CONTOURS && out.iter().all(|c| c.pieces.len() <= MAX_SEGMENTS)).then_some(out)
}

fn lerp(a: DVec2, b: DVec2, t: f64) -> DVec2 {
    a + (b - a) * t
}

/// The cubic from `p0` through `piece` cut at `t`: the two halves.
fn split(p0: DVec2, piece: [DVec2; 3], t: f64) -> ([DVec2; 3], [DVec2; 3]) {
    let [p1, p2, p3] = piece;
    let (a, b, c) = (lerp(p0, p1, t), lerp(p1, p2, t), lerp(p2, p3, t));
    let (d, e) = (lerp(a, b, t), lerp(b, c, t));
    let f = lerp(d, e, t);
    ([a, d, f], [e, c, p3])
}

fn length(p0: DVec2, piece: [DVec2; 3]) -> f64 {
    (piece[0] - p0).length() + (piece[1] - piece[0]).length() + (piece[2] - piece[1]).length()
}

/// Splits the longest piece in two until the contour has `count` pieces.
fn grow(contour: &mut Contour, count: usize) {
    while contour.pieces.len() < count {
        let mut from = contour.start;
        let mut longest = (0, f64::MIN, contour.start);
        for (index, piece) in contour.pieces.iter().enumerate() {
            let l = length(from, *piece);
            if l > longest.1 {
                longest = (index, l, from);
            }
            from = piece[2];
        }
        let (index, _, p0) = longest;
        let (first, second) = split(p0, contour.pieces[index], 0.5);
        contour.pieces[index] = first;
        contour.pieces.insert(index + 1, second);
    }
}

/// The contour shrunk to one point, with as many pieces as `like`.
fn collapsed(like: &Contour) -> Contour {
    let sum: DVec2 = like.pieces.iter().map(|p| p[2]).sum();
    let middle = sum / like.pieces.len() as f64;
    Contour {
        start: middle,
        pieces: vec![[middle; 3]; like.pieces.len()],
        closed: like.closed,
    }
}

fn area(contour: &Contour) -> f64 {
    let mut total = 0.0;
    let mut from = contour.start;
    for piece in &contour.pieces {
        total += from.perp_dot(piece[2]);
        from = piece[2];
    }
    total + from.perp_dot(contour.start)
}

fn reversed(contour: &Contour) -> Contour {
    let mut ends = vec![contour.start];
    ends.extend(contour.pieces.iter().map(|p| p[2]));
    let pieces = contour
        .pieces
        .iter()
        .enumerate()
        .rev()
        .map(|(i, piece)| [piece[1], piece[0], ends[i]])
        .collect();
    Contour {
        start: *ends.last().unwrap_or(&contour.start),
        pieces,
        closed: contour.closed,
    }
}

/// `b` turned to start where its points are nearest `a`'s, and flipped if it winds the other way.
fn align(a: &Contour, b: &Contour) -> Contour {
    if !(a.closed && b.closed) {
        return b.clone();
    }
    let mut b = b.clone();
    if area(a) * area(&b) < 0.0 {
        b = reversed(&b);
    }
    if b.pieces.len() > MAX_TURN_SEARCH {
        return b;
    }
    let count = b.pieces.len();
    let cost = |turn: usize| -> f64 {
        (0..count)
            .map(|i| {
                let end = b.pieces[(i + turn) % count][2];
                (a.pieces[i][2] - end).length_squared()
            })
            .sum()
    };
    let best = (0..count)
        .min_by(|x, y| cost(*x).total_cmp(&cost(*y)))
        .unwrap_or(0);
    if best == 0 {
        return b;
    }
    let start = b.pieces[best - 1][2];
    let mut pieces = b.pieces[best..].to_vec();
    pieces.extend_from_slice(&b.pieces[..best]);
    Contour {
        start,
        pieces,
        closed: true,
    }
}

fn mix(a: Rgba8, b: Rgba8, t: f64) -> Rgba8 {
    let channel = |x: u8, y: u8| (f64::from(x) + (f64::from(y) - f64::from(x)) * t).round() as u8;
    Rgba8::new(
        channel(a.r, b.r),
        channel(a.g, b.g),
        channel(a.b, b.b),
        channel(a.a, b.a),
    )
}

fn faded(color: Rgba8, by: f64) -> Rgba8 {
    Rgba8::new(
        color.r,
        color.g,
        color.b,
        (f64::from(color.a) * by).round() as u8,
    )
}

fn point(p: DVec2) -> Option<DocumentPoint> {
    DocumentPoint::new(p.x, p.y).ok()
}

/// The shape `t` of the way from `a` to `b` (0 is `a`, 1 is `b`), under `a`'s identity so it can
/// be picked on the stage. `None` when the two cannot be matched (a closed contour against an open
/// one, or more than the limits), in which case the caller holds `a`.
pub fn morph(a: &Shape, b: &Shape, t: f64) -> Option<Shape> {
    let t = t.clamp(0.0, 1.0);
    let (mut from, mut to) = (contours(a.path())?, contours(b.path())?);
    if from.is_empty() || to.is_empty() {
        return None;
    }
    let count = from.len().max(to.len());
    let mut pairs: Vec<(Contour, Contour)> = Vec::with_capacity(count);
    for index in 0..count {
        let (left, right) = match (from.get(index), to.get(index)) {
            (Some(l), Some(r)) => (l.clone(), r.clone()),
            (Some(l), None) => (l.clone(), collapsed(l)),
            (None, Some(r)) => (collapsed(r), r.clone()),
            (None, None) => return None,
        };
        pairs.push((left, right));
    }
    from.clear();
    to.clear();
    let mut path = Vec::new();
    for (mut left, mut right) in pairs {
        if left.closed != right.closed {
            return None;
        }
        let pieces = left.pieces.len().max(right.pieces.len());
        if pieces > MAX_SEGMENTS {
            return None;
        }
        grow(&mut left, pieces);
        grow(&mut right, pieces);
        let right = align(&left, &right);
        path.push(PathSegment::MoveTo(point(lerp(
            left.start,
            right.start,
            t,
        ))?));
        for (l, r) in left.pieces.iter().zip(&right.pieces) {
            path.push(PathSegment::CubicTo(
                point(lerp(l[0], r[0], t))?,
                point(lerp(l[1], r[1], t))?,
                point(lerp(l[2], r[2], t))?,
            ));
        }
        if left.closed {
            path.push(PathSegment::Close);
        }
    }
    let flat = |s: &Shape| s.flat_fill();
    let mut made = Shape::new(a.id(), path, mix(flat(a), flat(b), t)).ok()?;
    if let (Some(x), Some(y)) = (a.gradient(), b.gradient()) {
        let g = Gradient::linear(
            point(lerp(x.from().as_vec(), y.from().as_vec(), t))?,
            point(lerp(x.to().as_vec(), y.to().as_vec(), t))?,
            mix(x.start(), y.start(), t),
            mix(x.end(), y.end(), t),
        )
        .ok()?;
        made = made.with_gradient(g);
    }
    let stroke = match (a.stroke(), b.stroke()) {
        (Some(x), Some(y)) => {
            let source = if t < 0.5 { &x } else { &y };
            Some(
                Stroke::new(
                    x.width() + (y.width() - x.width()) * t,
                    mix(x.color(), y.color(), t),
                )
                .ok()?
                .with_cap(source.cap())
                .with_join(source.join()),
            )
        }
        (Some(x), None) => Some(
            Stroke::new(x.width(), faded(x.color(), 1.0 - t))
                .ok()?
                .with_cap(x.cap())
                .with_join(x.join()),
        ),
        (None, Some(y)) => Some(
            Stroke::new(y.width(), faded(y.color(), t))
                .ok()?
                .with_cap(y.cap())
                .with_join(y.join()),
        ),
        (None, None) => None,
    };
    if let Some(stroke) = stroke {
        made = made.with_stroke(stroke);
    }
    Some(made)
}
