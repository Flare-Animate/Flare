//! Offset Path and Outline Stroke.

use kurbo::{BezPath, PathEl, Stroke, StrokeOpts};
use crate::geom::{FillRule, PathData, SubPath};

pub use kurbo::{Cap, Join};

use linesweeper::topology::{ContourIdx, Contours};

use crate::boolean::{Arrangement, DEFAULT_PRECISION, Tidy, all_contours_to_path, contours_to_path, fill_bezpath, normalize_bez};

/// Flattening/fit tolerance handed to kurbo's stroker.
const STROKE_TOL: f64 = 1e-3;

fn stroke_bez(bp: &BezPath, width: f64, cap: Cap, join: Join, miter_limit: f64) -> BezPath {
    let style = Stroke::new(width).with_join(join).with_caps(cap).with_miter_limit(miter_limit.max(1.0));
    close_all(&kurbo::stroke(bp.iter(), &style, &StrokeOpts::default(), STROKE_TOL))
}

/// kurbo's stroker may leave the outline of an open path without a final `ClosePath`.
fn close_all(bp: &BezPath) -> BezPath {
    let mut out = BezPath::new();
    let mut open = false;
    for el in bp.iter() {
        match el {
            PathEl::MoveTo(_) => {
                if open {
                    out.close_path();
                }
                open = true;
            }
            PathEl::ClosePath => open = false,
            _ => {}
        }
        out.push(el);
    }
    if open {
        out.close_path();
    }
    out
}

/// Object → Path → Outline Stroke: the filled area painted by stroking `path` with `width`.
/// Overlaps produced by the stroker are removed, so the result is a clean compound path.
pub fn outline_stroke(path: &PathData, width: f64, cap: Cap, join: Join, miter_limit: f64) -> PathData {
    if width <= 0.0 || !width.is_finite() || path.is_empty() {
        return PathData::default();
    }
    stroke_region(&[stroke_bez(&path.to_bezpath(), width, cap, join, miter_limit)])
}

/// The area covered by stroke outlines (each filled with the non-zero rule, open subpaths
/// closed) as one clean path: overlaps removed and the stroker's pieces refitted.
pub fn stroke_region(outlines: &[BezPath]) -> PathData {
    let tidy = Tidy::free(DEFAULT_PRECISION);
    let parts: Vec<BezPath> = outlines.iter().map(close_all).filter(|o| !o.elements().is_empty()).collect();
    if let [one] = &parts[..] {
        return normalize_bez(one, FillRule::NonZero).map(|c| all_contours_to_path(&c, &tidy)).unwrap_or_default();
    }
    // Each outline is normalised on its own first: a hollow arrowhead may wind against the line
    // it overlaps, and the raw windings would cancel there under the non-zero rule.
    let mut all = BezPath::new();
    for p in &parts {
        if let Ok(c) = normalize_bez(p, FillRule::NonZero) {
            for ct in c.contours() {
                all.extend(ct.path.iter());
            }
        }
    }
    if all.elements().is_empty() {
        return PathData::default();
    }
    normalize_bez(&all, FillRule::NonZero).map(|c| all_contours_to_path(&c, &tidy)).unwrap_or_default()
}

/// Object → Path → Offset Path. Positive `delta` grows the filled area, negative insets it.
/// Closed subpaths are treated as a non-zero filled region; open subpaths are outlined with a
/// stroke of width `2|delta|` (butt caps).
pub fn offset_path(path: &PathData, delta: f64, join: Join, miter_limit: f64) -> PathData {
    if !delta.is_finite() || path.is_empty() {
        return PathData::default();
    }
    let closed = PathData::new(path.subpaths.iter().filter(|s| s.closed && s.anchors.len() > 1).cloned().collect());
    let open: Vec<SubPath> = path.subpaths.iter().filter(|s| !s.closed && s.anchors.len() > 1).cloned().collect();
    let d = delta.abs();
    let fill = fill_bezpath(&closed);
    let ring = if d > 0.0 { stroke_bez(&closed.to_bezpath(), 2.0 * d, Cap::Butt, join, miter_limit) } else { BezPath::new() };
    let open_bp = if d > 0.0 && !open.is_empty() {
        stroke_bez(&PathData::new(open).to_bezpath(), 2.0 * d, Cap::Butt, join, miter_limit)
    } else {
        BezPath::new()
    };
    let Ok(arr) = Arrangement::new(vec![(fill, FillRule::NonZero), (ring, FillRule::NonZero), (open_bp, FillRule::NonZero)]) else {
        return PathData::default();
    };
    let c = if delta >= 0.0 { arr.contours(|m| m[0] || m[1] || m[2]) } else { arr.contours(|m| m[0] && !m[1]) };
    // When `d` exceeds a curvature radius the stroker's inner offset inverts and its loops cancel
    // winding, leaving faces uncovered that are really within `d` of the path. Every face of the
    // true offset is bounded by arrangement edges, so a contour is spurious iff a point deep inside
    // it lies closer than `d`: holes when growing, islands when insetting.
    let src = path.to_bezpath();
    let drop_outer = delta < 0.0;
    let spurious: Vec<bool> =
        c.contours().map(|k| k.outer == drop_outer && deep_point(&k.path).is_some_and(|p| dist_to(&src, p) < d * (1.0 - 1e-4) - 1e-6)).collect();
    if !spurious.contains(&true) {
        return all_contours_to_path(&c, &Tidy::free(DEFAULT_PRECISION));
    }
    let keep = (0..spurious.len()).filter(|&i| !has_marked_ancestor(&c, i, &spurious)).map(ContourIdx);
    contours_to_path(&c, keep, &Tidy::free(DEFAULT_PRECISION))
}

fn has_marked_ancestor(c: &Contours, mut i: usize, marked: &[bool]) -> bool {
    loop {
        if marked[i] {
            return true;
        }
        match c[ContourIdx(i)].parent {
            Some(ContourIdx(p)) => i = p,
            None => return false,
        }
    }
}

/// A point well inside a closed contour: the midpoint of the widest span on the horizontal line
/// through its vertical centre.
fn deep_point(bp: &BezPath) -> Option<kurbo::Point> {
    use kurbo::Shape;
    let r = bp.bounding_box();
    if !(r.width() > 0.0 && r.height() > 0.0) {
        return None;
    }
    let mut best: Option<(f64, kurbo::Point)> = None;
    for f in [0.5, 0.3, 0.7] {
        let y = r.y0 + r.height() * f;
        let mut xs = vec![];
        let mut prev: Option<kurbo::Point> = None;
        let mut start = kurbo::Point::ZERO;
        let edge = |a: kurbo::Point, b: kurbo::Point, xs: &mut Vec<f64>| {
            if (a.y <= y) != (b.y <= y) {
                xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
            }
        };
        kurbo::flatten(bp.iter(), 0.01 * r.width().min(r.height()).max(1e-6), |el| match el {
            PathEl::MoveTo(p) => {
                start = p;
                prev = Some(p);
            }
            PathEl::LineTo(p) => {
                if let Some(a) = prev {
                    edge(a, p, &mut xs);
                }
                prev = Some(p);
            }
            PathEl::ClosePath => {
                if let Some(a) = prev {
                    edge(a, start, &mut xs);
                }
                prev = Some(start);
            }
            _ => {}
        });
        xs.sort_by(f64::total_cmp);
        for w in xs.as_chunks::<2>().0 {
            let span = w[1] - w[0];
            if best.is_none_or(|(b, _)| span > b) {
                best = Some((span, kurbo::Point::new((w[0] + w[1]) / 2.0, y)));
            }
        }
    }
    best.map(|(_, p)| p)
}

fn dist_to(bp: &BezPath, p: kurbo::Point) -> f64 {
    use kurbo::ParamCurveNearest;
    bp.segments().map(|s| s.nearest(p, 1e-6).distance_sq).fold(f64::INFINITY, f64::min).sqrt()
}

#[cfg(test)]
mod tests {
    use kurbo::Shape;

    use super::*;

    #[test]
    fn stroke_region_unites_outlines_whatever_their_winding() {
        // A bar, and a square ring wound the other way that the bar runs into.
        let bar = kurbo::Rect::new(0.0, 0.0, 20.0, 4.0).to_path(0.1);
        let mut ring = kurbo::Rect::new(10.0, -8.0, 26.0, 12.0).to_path(0.1).reverse_subpaths();
        ring.extend(kurbo::Rect::new(14.0, -4.0, 22.0, 8.0).to_path(0.1).iter());
        let region = stroke_region(&[bar, ring]);
        let area = crate::area(&region, FillRule::NonZero);
        // Ring 16·20 − 8·12 = 224, plus the bar outside it (10·4) and inside its hole (6·4).
        assert!((area - (224.0 + 40.0 + 24.0)).abs() < 1e-6, "{area}");
    }

    #[test]
    fn miter_offset_finishes_where_the_sweep_used_to_loop() {
        // Issue #1028: with linesweeper 0.4 the sweep over this miter offset never finished and
        // its memory grew without bound, freezing the app. A synthetic outline (found by search)
        // with the same failure as the reported glyph; the exact coordinates matter.
        const OUTLINE: &str = "M74.359137297823,82.83738194552835 L88.097432485874,103.04979803227248 \
            L67.06936731196308,105.95779886823588 L53.29382873396223,98.53943925153727 \
            L29.684713319694094,94.17996020306241 L41.65480824807079,75.639672165115 \
            Q47.491034578751446,52.12555333014565 46.157302824282816,55.67193079127421 \
            Q69.08365151786553,53.9608681393331 68.37499772324333,51.17242466680129 \
            Q74.4136309339628,75.13284223919301 71.3309887459222,73.69128869058812 Z";
        let path = PathData::from_bezpath(&BezPath::from_svg(OUTLINE).unwrap());
        let (tx, rx) = std::sync::mpsc::channel();
        let input = path.clone();
        std::thread::spawn(move || tx.send(offset_path(&input, 1.8350459854022054, Join::Miter, 4.0)));
        let Ok(out) = rx.recv_timeout(std::time::Duration::from_secs(30)) else {
            // the stuck sweep keeps allocating on its thread: end the test binary now
            eprintln!("Offset Path didn't finish in 30 s (issue #1028)");
            std::process::exit(101);
        };
        let (before, after) = (crate::area(&path, FillRule::NonZero), crate::area(&out, FillRule::NonZero));
        assert_eq!(out.subpaths.len(), 1);
        assert!(after > before, "{before} → {after}");
    }
}
