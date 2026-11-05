//! Editing one segment of a curve by its handles: which point a pixel grabs, and where the
//! handles go when one is dragged, under the rules of each handle kind.
//!
//! The segment is the unit square: it starts at (0, 0) and ends at (1, 1). `handles` is
//! `[right.dt, right.dv, left.dt, left.dv]`, offsets from the start and end keys (the layout
//! `flare-document`'s `SpanInterp` stores). Pure geometry: nothing here draws or allocates per
//! call beyond the small arrays it returns.
//!
//! The kinds follow the documented behaviour of handle types in curve editors: an automatic
//! handle that is dragged stops being automatic (Auto and AutoClamped become Aligned, Vector
//! becomes Free), Aligned keeps both handles on one line, and Free moves one alone.

use crate::view::View;
use crate::{Curve, Extrapolation, HandleKind, Key, Params, TweenError};

/// The furthest a handle may reach in value, so a drag cannot run off to infinity.
pub const MAX_HANDLE_VALUE: f64 = 4.0;
/// The most points a sampled polyline has.
pub const MAX_SAMPLES: usize = 512;

/// What a pixel can grab on the segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Grab {
    /// The handle of the start key, which points into the segment.
    Right,
    /// The handle of the end key, which points back into the segment.
    Left,
}

/// The four anchor points of a segment in world coordinates: start key, its handle, the end
/// key's handle and the end key. The handles are absolute here, not offsets.
#[must_use]
pub fn points(handles: [f64; 4]) -> [(f64, f64); 4] {
    [
        (0.0, 0.0),
        (handles[0], handles[1]),
        (1.0 + handles[2], 1.0 + handles[3]),
        (1.0, 1.0),
    ]
}

/// The handles a kind gives a lone segment, which is what its editor shows before a drag.
/// Free and Aligned keep the ones given.
///
/// # Errors
/// Handles that are not finite.
pub fn effective_handles(kind: HandleKind, handles: [f64; 4]) -> Result<[f64; 4], TweenError> {
    let mut a = Key::new(0.0, 0.0);
    a.handle_kind = kind;
    a.right.dt = handles[0];
    a.right.dv = handles[1];
    let mut b = Key::new(1.0, 1.0);
    b.handle_kind = kind;
    b.left.dt = handles[2];
    b.left.dv = handles[3];
    let curve = Curve::new(vec![a, b], Extrapolation::Constant, Params::default())?;
    let keys = curve.keys();
    Ok([
        keys[0].right.dt,
        keys[0].right.dv,
        keys[1].left.dt,
        keys[1].left.dv,
    ])
}

/// The handle under a pixel, if any, within `radius` pixels. The nearer one wins a tie.
#[must_use]
pub fn hit_test(view: &View, handles: [f64; 4], pixel: (f64, f64), radius: f64) -> Option<Grab> {
    if !(pixel.0.is_finite() && pixel.1.is_finite()) || !radius.is_finite() {
        return None;
    }
    let p = points(handles);
    let dist = |world: (f64, f64)| {
        let s = view.to_screen(world);
        (s.0 - pixel.0).hypot(s.1 - pixel.1)
    };
    let (right, left) = (dist(p[1]), dist(p[2]));
    match (right <= radius, left <= radius) {
        (true, true) => Some(if right <= left {
            Grab::Right
        } else {
            Grab::Left
        }),
        (true, false) => Some(Grab::Right),
        (false, true) => Some(Grab::Left),
        (false, false) => None,
    }
}

/// How a drag is held in: a grid the handle snaps to, in world units.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DragOptions {
    pub snap: Option<f64>,
}

/// The result of a drag: the handle kind to store and the handles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dragged {
    pub kind: HandleKind,
    pub handles: [f64; 4],
}

fn snapped(value: f64, grid: Option<f64>) -> f64 {
    match grid {
        Some(g) if g.is_finite() && g > 0.0 => (value / g).round() * g,
        _ => value,
    }
}

/// Moves one handle so its point lands on `target` (world coordinates), under the rules of
/// `kind`. The time stays inside the segment and the value inside `MAX_HANDLE_VALUE`. A target
/// that is not finite changes nothing.
///
/// # Errors
/// `handles` that are not finite.
pub fn drag_handle(
    kind: HandleKind,
    handles: [f64; 4],
    grab: Grab,
    target: (f64, f64),
    options: DragOptions,
) -> Result<Dragged, TweenError> {
    let start = effective_handles(kind, handles)?;
    if !(target.0.is_finite() && target.1.is_finite()) {
        return Ok(Dragged {
            kind,
            handles: start,
        });
    }
    let target = (
        snapped(target.0, options.snap),
        snapped(target.1, options.snap),
    );
    let new_kind = match kind {
        HandleKind::Auto | HandleKind::AutoClamped => HandleKind::Aligned,
        HandleKind::Vector => HandleKind::Free,
        other => other,
    };
    let mut h = start;
    let v = |x: f64| x.clamp(-MAX_HANDLE_VALUE, MAX_HANDLE_VALUE);
    let (moved, other) = match grab {
        Grab::Right => (0, 2),
        Grab::Left => (2, 0),
    };
    let (dt, dv) = match grab {
        Grab::Right => (target.0.clamp(0.0, 1.0), v(target.1)),
        Grab::Left => ((target.0 - 1.0).clamp(-1.0, 0.0), v(target.1 - 1.0)),
    };
    h[moved] = dt;
    h[moved + 1] = dv;
    if new_kind == HandleKind::Aligned {
        // The other handle keeps its length and points the opposite way. When the moved handle
        // has no length there is no direction, so the other stays.
        let len = dt.hypot(dv);
        let other_len = h[other].hypot(h[other + 1]);
        if len > 0.0 {
            let (ox, oy) = (-dt * other_len / len, -dv * other_len / len);
            // Keep the other handle's time inside the segment, scaling the pair of its parts
            // together so the line stays straight.
            let shrink = if ox.abs() > 1.0 { 1.0 / ox.abs() } else { 1.0 };
            h[other] = ox * shrink;
            h[other + 1] = v(oy * shrink);
        }
    }
    Ok(Dragged {
        kind: new_kind,
        handles: h,
    })
}

/// Fills `out` with the curve's value at `n + 1` evenly spaced times from 0 to 1 (`n` is limited
/// to `MAX_SAMPLES`). The vector is cleared and reused, so a caller that keeps it allocates once.
pub fn sample_into(mut f: impl FnMut(f64) -> f64, n: usize, out: &mut Vec<f64>) {
    let n = n.clamp(1, MAX_SAMPLES);
    out.clear();
    out.extend((0..=n).map(|i| {
        let v = f(i as f64 / n as f64);
        if v.is_finite() { v } else { 0.0 }
    }));
}
