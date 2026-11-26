//! Fewer points in a stroke's outline, the same outline. The smooth centre line is cut at least
//! [`super::curve`]'s four pieces between two input samples, which are a pixel or so apart: a
//! point every quarter pixel even where the line runs straight, most of the size of a drawing in
//! the file. A point both sides of the stroke pass within a fiftieth of a stage unit of (the sag
//! a round end is allowed too) adds nothing anyone can see, and is left out.

use glamx::DVec2;

/// How far, in stage units, an edge of the outline may move when points are left out: the same
/// as a round end may stray from its circle.
pub(super) const OUTLINE_SAG: f64 = 0.02;
/// The most of the width at a point either edge may give up, so a hairline narrower than the
/// sag keeps its shape.
const MAX_WIDTH_SHARE: f64 = 0.1;

/// How far `p` is from the segment `a`-`b`.
fn off(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let length_sq = ab.length_squared();
    let t = if length_sq > f64::EPSILON {
        ((p - a).dot(ab) / length_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (a + ab * t)).length()
}

/// The indices of the points of the two sides kept: the same for both, so `left[i]` still faces
/// `right[i]`, and every point left out lies within [`OUTLINE_SAG`] (and a tenth of the width
/// there) of both sides as they are kept. The first and last are always kept. Sides of unequal
/// length are kept whole.
pub(super) fn thin(left: &[DVec2], right: &[DVec2]) -> Vec<usize> {
    let n = left.len();
    if n < 3 || right.len() != n {
        return (0..n).collect();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0, n - 1)];
    while let Some((from, to)) = stack.pop() {
        let worst = (from + 1..to)
            .map(|i| {
                let width = (left[i] - right[i]).length();
                let allowed = OUTLINE_SAG.min(width * MAX_WIDTH_SHARE);
                let strays =
                    off(left[i], left[from], left[to]).max(off(right[i], right[from], right[to]));
                (i, strays - allowed)
            })
            .max_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((i, excess)) = worst
            && excess > 0.0
        {
            keep[i] = true;
            stack.push((from, i));
            stack.push((i, to));
        }
    }
    (0..n).filter(|&i| keep[i]).collect()
}

#[cfg(test)]
#[path = "thin_tests.rs"]
mod tests;
