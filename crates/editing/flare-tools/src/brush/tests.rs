#![allow(clippy::expect_used)]

use flare_input::PointerKind;

use super::*;

fn sample(x: f64, y: f64, pressure: f32) -> InputSample {
    InputSample::new(x, y, pressure, PointerKind::Pen).expect("sample")
}

fn points(path: &[PathSegment]) -> Vec<DVec2> {
    path.iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some(p.as_vec()),
            _ => None,
        })
        .collect()
}

/// The vertical extent of the outline near `x`.
fn thickness_at(path: &[PathSegment], x: f64) -> f64 {
    let near: Vec<f64> = points(path)
        .into_iter()
        .filter(|p| (p.x - x).abs() < 2.5)
        .map(|p| p.y)
        .collect();
    near.iter().copied().fold(f64::MIN, f64::max) - near.iter().copied().fold(f64::MAX, f64::min)
}

#[test]
fn a_mouse_stroke_is_one_width_all_along() {
    let samples: Vec<_> = (0..=20)
        .map(|i| sample(f64::from(i) * 5.0, 0.0, 1.0))
        .collect();
    let path = outline(&samples, 10.0, PressureCurve::default(), 0.5).expect("outline");
    assert!(matches!(path.last(), Some(PathSegment::Close)));
    for x in [20.0, 50.0, 80.0] {
        assert!((thickness_at(&path, x) - 10.0).abs() < 0.5, "x = {x}");
    }
}

#[test]
fn pressing_harder_makes_the_stroke_wider() {
    // Pressure rises from nothing to full along the stroke.
    let samples: Vec<_> = (0..=20)
        .map(|i| sample(f64::from(i) * 5.0, 0.0, i as f32 / 20.0))
        .collect();
    let curve = PressureCurve::new(0.1, 1.0);
    let path = outline(&samples, 20.0, curve, 0.5).expect("outline");
    let (light, firm) = (thickness_at(&path, 20.0), thickness_at(&path, 80.0));
    assert!(firm > light * 2.0, "{light} then {firm}");
    assert!(
        light >= 20.0 * 0.1 - 0.5,
        "never thinner than the curve's minimum"
    );
}

#[test]
fn the_ends_are_round_so_the_stroke_reaches_past_its_first_and_last_points() {
    let samples = [sample(10.0, 10.0, 1.0), sample(60.0, 10.0, 1.0)];
    let path = outline(&samples, 10.0, PressureCurve::default(), 0.5).expect("outline");
    let xs: Vec<f64> = points(&path).iter().map(|p| p.x).collect();
    let (lo, hi) = (
        xs.iter().copied().fold(f64::MAX, f64::min),
        xs.iter().copied().fold(f64::MIN, f64::max),
    );
    assert!(
        (lo - 5.0).abs() < 0.2 && (hi - 65.0).abs() < 0.2,
        "{lo}..{hi}"
    );
}

#[test]
fn a_click_a_bad_width_and_a_straight_run_of_points_behave() {
    let one = [sample(1.0, 1.0, 1.0), sample(1.0, 1.0, 0.5)];
    assert!(
        outline(&one, 5.0, PressureCurve::default(), 0.5).is_none(),
        "no movement"
    );
    let two = [sample(0.0, 0.0, 1.0), sample(9.0, 0.0, 1.0)];
    for width in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert!(
            outline(&two, width, PressureCurve::default(), 0.5).is_none(),
            "{width}"
        );
    }
    // A hundred points on a line simplify to the ends.
    let line: Vec<_> = (0..100).map(|i| sample(f64::from(i), 0.0, 1.0)).collect();
    let short = outline(&line, 4.0, PressureCurve::default(), 0.5).expect("outline");
    assert!(short.len() < 60, "{} segments", short.len());
}
