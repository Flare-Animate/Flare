//! Following a path: where a point that travels along it is after a fraction of its length.

use flare_document::PathSegment;
use glamx::DVec2;

/// Straight pieces per curve when a path is flattened.
const CURVE_STEPS: usize = 24;

/// The path as one polyline per subpath; each is closed by repeating its first point when the
/// path closes it.
pub fn flatten(path: &[PathSegment]) -> Vec<Vec<DVec2>> {
    let mut lines: Vec<Vec<DVec2>> = Vec::new();
    for segment in path {
        match segment {
            PathSegment::MoveTo(p) => lines.push(vec![p.as_vec()]),
            PathSegment::LineTo(p) => {
                if let Some(line) = lines.last_mut() {
                    line.push(p.as_vec());
                }
            }
            PathSegment::CubicTo(a, b, c) => {
                if let Some(line) = lines.last_mut()
                    && let Some(&from) = line.last()
                {
                    let (a, b, c) = (a.as_vec(), b.as_vec(), c.as_vec());
                    for step in 1..=CURVE_STEPS {
                        let t = step as f64 / CURVE_STEPS as f64;
                        let u = 1.0 - t;
                        line.push(
                            from * (u * u * u)
                                + a * (3.0 * u * u * t)
                                + b * (3.0 * u * t * t)
                                + c * (t * t * t),
                        );
                    }
                }
            }
            PathSegment::Close => {
                if let Some(line) = lines.last_mut()
                    && let Some(&first) = line.first()
                {
                    line.push(first);
                }
            }
        }
    }
    lines
}

/// The point `fraction` (0 to 1) of the way along the path's first subpath, by length. `None`
/// when the path has no point at all; a path with no length is just its first point.
pub fn point_along(path: &[PathSegment], fraction: f64) -> Option<DVec2> {
    let line = flatten(path).into_iter().next()?;
    let first = *line.first()?;
    let total: f64 = line.windows(2).map(|w| (w[1] - w[0]).length()).sum();
    if total <= f64::EPSILON {
        return Some(first);
    }
    let mut remaining = fraction.clamp(0.0, 1.0) * total;
    for pair in line.windows(2) {
        let length = (pair[1] - pair[0]).length();
        if remaining <= length {
            let t = if length > 0.0 {
                remaining / length
            } else {
                0.0
            };
            return Some(pair[0].lerp(pair[1], t));
        }
        remaining -= length;
    }
    line.last().copied()
}
