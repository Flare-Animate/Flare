//! The geometric stroke styles: hatching, a ragged edge, herringbone ribs, a rope and a zigzag,
//! built along a line's own path. The pattern is made in the shape's local units and then drawn
//! with the shape's transform, like the outline it stands for, so it scales with the shape.

use flare_document::StrokeStyle;
use vello_cpu::kurbo::{BezPath, PathEl, flatten};

/// Most samples taken along one stroke in all. A long line is sampled more coarsely rather than
/// producing more pattern than this, so the work per shape is bounded.
const MAX_SAMPLES: f64 = 4000.0;
/// Most separate lines (subpaths) a pattern is built on; the rest of a huge path is left plain.
const MAX_LINES: usize = 512;
/// Widths are clamped to this range: a hairline still gets a visible pattern, and a huge width
/// cannot make the pattern cover the page with an unbounded number of marks.
const MIN_WIDTH: f64 = 1.0;
const MAX_WIDTH: f64 = 10_000.0;

/// The path to draw for a geometric `style` on `path`, `width` wide, in the same local units.
/// Hatched and Herringbone keep the line itself and add to it; Ragged, Rope and Zigzag replace it.
/// `None` for the styles that are a dash pattern or solid, which are drawn from the path directly.
pub(crate) fn pattern(path: &BezPath, width: f64, style: StrokeStyle) -> Option<BezPath> {
    if !style.is_geometric() {
        return None;
    }
    let w = if width.is_finite() {
        width.clamp(MIN_WIDTH, MAX_WIDTH)
    } else {
        MIN_WIDTH
    };
    let mut out = BezPath::new();
    if matches!(style, StrokeStyle::Hatched | StrokeStyle::Herringbone) {
        out.extend(path.elements().iter().copied());
    }
    let lines = lines(path, (w * 0.02).max(1e-3));
    let total: f64 = lines.iter().map(Line::length).sum();
    if !total.is_finite() || total <= 0.0 {
        return Some(out);
    }
    let base = match style {
        StrokeStyle::Hatched => 1.5,
        StrokeStyle::Ragged => 1.0,
        StrokeStyle::Herringbone => 2.0,
        StrokeStyle::Rope => 0.25,
        _ => 1.5,
    };
    let step = (base * w).max(total / MAX_SAMPLES);
    for (index, line) in lines.iter().enumerate() {
        let samples = line.samples(step);
        match style {
            StrokeStyle::Hatched => hatch(&mut out, &samples, w),
            StrokeStyle::Herringbone => ribs(&mut out, &samples, w),
            StrokeStyle::Ragged => ragged(&mut out, line, &samples, w, index),
            StrokeStyle::Rope => rope(&mut out, line, &samples, w),
            StrokeStyle::Zigzag => zigzag(&mut out, line, &samples, w),
            _ => {}
        }
    }
    Some(out)
}

/// One subpath as a list of distinct, finite points.
struct Line {
    points: Vec<[f64; 2]>,
    closed: bool,
}

/// A point on the line, where the line runs (`tangent`, a unit vector), at `along` from its start.
struct Sample {
    point: [f64; 2],
    tangent: [f64; 2],
    along: f64,
}

impl Line {
    fn length(&self) -> f64 {
        self.segments().map(|(a, b)| dist(a, b)).sum()
    }

    /// The points one segment apart, from the start of the line, and the last point of an open
    /// line. Empty for a line with no length.
    fn samples(&self, step: f64) -> Vec<Sample> {
        let segments: Vec<([f64; 2], [f64; 2], f64)> =
            self.segments().map(|(a, b)| (a, b, dist(a, b))).collect();
        let total: f64 = segments.iter().map(|s| s.2).sum();
        if !(step > 0.0 && total > 0.0 && total.is_finite()) {
            return Vec::new();
        }
        let end = if self.closed {
            total
        } else {
            total + step * 1e-9
        };
        let mut out = Vec::new();
        let (mut k, mut seg_start, mut along) = (0, 0.0, 0.0);
        while along < end {
            while k + 1 < segments.len() && along > seg_start + segments[k].2 {
                seg_start += segments[k].2;
                k += 1;
            }
            let (a, b, len) = segments[k];
            let f = ((along - seg_start) / len).clamp(0.0, 1.0);
            out.push(Sample {
                point: [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f],
                tangent: [(b[0] - a[0]) / len, (b[1] - a[1]) / len],
                along,
            });
            along += step;
        }
        out
    }

    /// Each segment as a pair of points, the closing one included for a closed line.
    fn segments(&self) -> impl Iterator<Item = ([f64; 2], [f64; 2])> + '_ {
        let n = self.points.len();
        let count = if self.closed { n } else { n.saturating_sub(1) };
        (0..count).map(move |i| (self.points[i], self.points[(i + 1) % n]))
    }
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (b[0] - a[0]).hypot(b[1] - a[1])
}

/// The subpaths of `path`, flattened to within `tolerance`, with repeated and non-finite points
/// dropped. A line with fewer than two points is not kept.
fn lines(path: &BezPath, tolerance: f64) -> Vec<Line> {
    let mut lines: Vec<Line> = Vec::new();
    let mut current: Option<Line> = None;
    fn finish(current: &mut Option<Line>, lines: &mut Vec<Line>) {
        if let Some(mut line) = current.take() {
            if line.closed && line.points.len() > 1 && line.points.first() == line.points.last() {
                line.points.pop();
            }
            if line.points.len() >= 2 && lines.len() < MAX_LINES {
                lines.push(line);
            }
        }
    }
    flatten(path.elements().iter().copied(), tolerance, |el| match el {
        PathEl::MoveTo(p) => {
            finish(&mut current, &mut lines);
            current = Some(Line {
                points: Vec::new(),
                closed: false,
            });
            push_point(current.as_mut(), p);
        }
        PathEl::LineTo(p) => push_point(current.as_mut(), p),
        PathEl::ClosePath => {
            if let Some(line) = current.as_mut() {
                line.closed = true;
            }
            finish(&mut current, &mut lines);
        }
        // Flattening leaves only moves, lines and closes.
        PathEl::QuadTo(..) | PathEl::CurveTo(..) => {}
    });
    finish(&mut current, &mut lines);
    lines
}

fn push_point(line: Option<&mut Line>, p: vello_cpu::kurbo::Point) {
    let Some(line) = line else { return };
    let p = [p.x, p.y];
    if !(p[0].is_finite() && p[1].is_finite()) {
        return;
    }
    if line.points.last().is_some_and(|last| dist(*last, p) < 1e-9) {
        return;
    }
    line.points.push(p);
}

/// Short strokes across the line, each slanted 45 degrees from its normal, `1.5 w` either side.
fn hatch(out: &mut BezPath, samples: &[Sample], w: f64) {
    for s in samples {
        let [tx, ty] = s.tangent;
        let (nx, ny) = (-ty, tx);
        let (dx, dy) = (nx + tx, ny + ty);
        let len = dx.hypot(dy).max(f64::MIN_POSITIVE);
        let h = 1.5 * w / len;
        let [x, y] = s.point;
        out.move_to((x - dx * h, y - dy * h));
        out.line_to((x + dx * h, y + dy * h));
    }
}

/// Ribs slanting back from every other sample on alternate sides.
fn ribs(out: &mut BezPath, samples: &[Sample], w: f64) {
    for (i, s) in samples.iter().enumerate() {
        let side = if i % 2 == 0 { 1.0 } else { -1.0 };
        let [tx, ty] = s.tangent;
        let (nx, ny) = (-ty, tx);
        let r = 1.2 * w;
        let [x, y] = s.point;
        out.move_to((x, y));
        out.line_to((x - tx * r + nx * side * r, y - ty * r + ny * side * r));
    }
}

/// The line, each point pushed off it by a deterministic amount within `0.8 w`.
fn ragged(out: &mut BezPath, line: &Line, samples: &[Sample], w: f64, index: usize) {
    let offsets = samples.iter().enumerate().map(|(i, s)| {
        let jitter = unit_noise((index as u64) << 32 | i as u64);
        let [tx, ty] = s.tangent;
        let [x, y] = s.point;
        (x - ty * 0.8 * w * jitter, y + tx * 0.8 * w * jitter)
    });
    polyline(out, line.closed, offsets);
}

/// Two strands twisting around the line, half a turn apart, `w / 2` either side of it.
fn rope(out: &mut BezPath, line: &Line, samples: &[Sample], w: f64) {
    for strand in [0.0, std::f64::consts::PI] {
        let points = samples.iter().map(|s| {
            let phase = std::f64::consts::PI * s.along / w + strand;
            let offset = 0.5 * w * phase.sin();
            let [tx, ty] = s.tangent;
            let [x, y] = s.point;
            (x - ty * offset, y + tx * offset)
        });
        polyline(out, line.closed, points);
    }
}

/// The line replaced by points alternately `0.75 w` either side of it, `1.5 w` apart.
fn zigzag(out: &mut BezPath, line: &Line, samples: &[Sample], w: f64) {
    let points = samples.iter().enumerate().map(|(i, s)| {
        let side = w * if i % 2 == 0 { 0.75 } else { -0.75 };
        let [tx, ty] = s.tangent;
        let [x, y] = s.point;
        (x - ty * side, y + tx * side)
    });
    polyline(out, line.closed, points);
}

fn polyline(out: &mut BezPath, closed: bool, points: impl Iterator<Item = (f64, f64)>) {
    for (i, p) in points.enumerate() {
        if i == 0 {
            out.move_to(p);
        } else {
            out.line_to(p);
        }
    }
    if closed {
        out.close_path();
    }
}

/// A repeatable value in `[-1, 1]` for `i`, from a 64-bit mix (SplitMix64), so a ragged line is
/// the same on every render.
fn unit_noise(i: u64) -> f64 {
    let mut x = i.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]
    use super::*;
    use flare_document::StrokeStyle as S;

    fn line(from: (f64, f64), to: (f64, f64)) -> BezPath {
        let mut p = BezPath::new();
        p.move_to(from);
        p.line_to(to);
        p
    }

    #[test]
    fn solid_and_dash_styles_have_no_geometric_pattern() {
        let path = line((0.0, 0.0), (100.0, 0.0));
        for style in [S::Solid, S::Dashed, S::Dotted, S::Stipple] {
            assert!(pattern(&path, 4.0, style).is_none(), "{style}");
        }
    }

    #[test]
    fn zigzag_replaces_the_line_with_points_on_both_sides() {
        let path = line((0.0, 0.0), (60.0, 0.0));
        let out = pattern(&path, 4.0, S::Zigzag).expect("pattern");
        let ys: Vec<f64> = out
            .elements()
            .iter()
            .filter_map(|el| match el {
                PathEl::MoveTo(p) | PathEl::LineTo(p) => Some(p.y),
                _ => None,
            })
            .collect();
        assert!(ys.len() > 10, "a zigzag has many points: {}", ys.len());
        assert!(ys.iter().any(|y| *y > 2.0) && ys.iter().any(|y| *y < -2.0));
        assert!(ys.iter().all(|y| y.abs() <= 0.75 * 4.0 + 1e-9));
    }

    #[test]
    fn hatched_and_herringbone_keep_the_line_and_add_to_it() {
        let path = line((0.0, 0.0), (60.0, 0.0));
        for style in [S::Hatched, S::Herringbone] {
            let out = pattern(&path, 4.0, style).expect("pattern");
            assert!(
                matches!(out.elements().first(), Some(PathEl::MoveTo(p)) if p.x == 0.0 && p.y == 0.0),
                "{style} starts with the line"
            );
            assert!(
                out.elements().len() > path.elements().len() + 10,
                "{style} adds marks"
            );
        }
    }

    #[test]
    fn rope_is_two_strands_that_stay_within_half_a_width() {
        let path = line((0.0, 0.0), (80.0, 0.0));
        let out = pattern(&path, 6.0, S::Rope).expect("pattern");
        let moves = out
            .elements()
            .iter()
            .filter(|el| matches!(el, PathEl::MoveTo(_)))
            .count();
        assert_eq!(moves, 2, "one move per strand");
        assert!(out.elements().iter().all(|el| match el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => p.y.abs() <= 0.5 * 6.0 + 1e-9,
            _ => true,
        }));
    }

    #[test]
    fn ragged_is_deterministic_and_bounded() {
        let path = line((0.0, 0.0), (50.0, 0.0));
        let a = pattern(&path, 3.0, S::Ragged).expect("pattern");
        let b = pattern(&path, 3.0, S::Ragged).expect("pattern");
        assert_eq!(a.elements(), b.elements(), "the same line every time");
        assert!(a.elements().iter().all(|el| match el {
            PathEl::MoveTo(p) | PathEl::LineTo(p) => p.y.abs() <= 0.8 * 3.0 + 1e-9,
            _ => true,
        }));
    }

    #[test]
    fn a_long_line_is_sampled_within_the_bound() {
        let path = line((0.0, 0.0), (1.0e6, 0.0));
        for style in [S::Hatched, S::Ragged, S::Herringbone, S::Rope, S::Zigzag] {
            let out = pattern(&path, 0.001, style).expect("pattern");
            assert!(
                out.elements().len() <= 4 * MAX_SAMPLES as usize + 16,
                "{style}: {} elements",
                out.elements().len()
            );
        }
    }

    #[test]
    fn hostile_input_gives_a_plain_or_empty_pattern_not_a_panic() {
        let path = line((0.0, 0.0), (10.0, 0.0));
        for width in [f64::NAN, f64::INFINITY, -3.0, 0.0, 1.0e12] {
            for style in [S::Hatched, S::Rope] {
                assert!(pattern(&path, width, style).is_some(), "{style} at {width}");
            }
        }
        let empty = BezPath::new();
        assert!(
            pattern(&empty, 4.0, S::Zigzag)
                .expect("pattern")
                .elements()
                .is_empty()
        );
        let mut dot = BezPath::new();
        dot.move_to((5.0, 5.0));
        assert!(
            pattern(&dot, 4.0, S::Hatched)
                .expect("pattern")
                .elements()
                .len()
                <= 1
        );
        let mut bad = BezPath::new();
        bad.move_to((f64::NAN, 0.0));
        bad.line_to((10.0, f64::INFINITY));
        assert!(
            pattern(&bad, 4.0, S::Ragged)
                .expect("pattern")
                .elements()
                .is_empty()
        );
    }

    #[test]
    fn a_closed_loop_is_patterned_all_the_way_round() {
        let mut square = BezPath::new();
        square.move_to((0.0, 0.0));
        square.line_to((40.0, 0.0));
        square.line_to((40.0, 40.0));
        square.line_to((0.0, 40.0));
        square.close_path();
        let out = pattern(&square, 2.0, S::Zigzag).expect("pattern");
        let far = out
            .elements()
            .iter()
            .filter_map(|el| match el {
                PathEl::LineTo(p) => Some(p),
                _ => None,
            })
            .filter(|p| p.x < 0.0 || p.y < 0.0 || p.x > 40.0 || p.y > 40.0)
            .count();
        assert!(
            far > 0,
            "the zigzag runs off the sides of the square, both ways"
        );
    }
}
