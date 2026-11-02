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

fn run(from: (f64, f64), to: (f64, f64)) -> Vec<InputSample> {
    (0..=20)
        .map(|i| {
            let t = f64::from(i) / 20.0;
            sample(
                from.0 + (to.0 - from.0) * t,
                from.1 + (to.1 - from.1) * t,
                1.0,
            )
        })
        .collect()
}

fn height(path: &[PathSegment]) -> f64 {
    let ys: Vec<f64> = points(path).iter().map(|p| p.y).collect();
    ys.iter().copied().fold(f64::MIN, f64::max) - ys.iter().copied().fold(f64::MAX, f64::min)
}

fn horizontal_extent_near(path: &[PathSegment], y: f64) -> f64 {
    let near: Vec<f64> = points(path)
        .into_iter()
        .filter(|p| (p.y - y).abs() < 2.5)
        .map(|p| p.x)
        .collect();
    near.iter().copied().fold(f64::MIN, f64::max) - near.iter().copied().fold(f64::MAX, f64::min)
}

#[test]
fn a_round_full_nib_paints_the_same_as_the_plain_brush() {
    let samples = run((0.0, 0.0), (100.0, 40.0));
    let plain = outline(&samples, 12.0, PressureCurve::default(), 0.5);
    let nibbed = outline_with(&samples, 12.0, PressureCurve::default(), 0.5, Nib::ROUND);
    assert_eq!(plain, nibbed);
    assert!(plain.is_some());
}

#[test]
fn a_flat_nib_is_thick_across_its_long_axis_and_thin_along_it() {
    let flat = Nib::new(NibShape::Round, 0.0, 10.0);
    let curve = PressureCurve::default();
    let across = outline_with(&run((50.0, 0.0), (50.0, 100.0)), 20.0, curve, 0.5, flat)
        .expect("vertical stroke");
    let along = outline_with(&run((0.0, 50.0), (100.0, 50.0)), 20.0, curve, 0.5, flat)
        .expect("horizontal stroke");
    // Its long axis is horizontal: a vertical stroke is as wide as the nib, a horizontal one is
    // a tenth of that.
    assert!((horizontal_extent_near(&across, 50.0) - 20.0).abs() < 0.5);
    assert!((thickness_at(&along, 50.0) - 2.0).abs() < 0.3);
}

#[test]
fn turning_the_nib_turns_the_stroke() {
    let curve = PressureCurve::default();
    let flat_up = Nib::new(NibShape::Round, 90.0, 10.0);
    let horizontal =
        outline_with(&run((0.0, 50.0), (100.0, 50.0)), 20.0, curve, 0.5, flat_up).expect("stroke");
    // The long axis now points up, so a horizontal stroke is as thick as the nib is long.
    assert!((thickness_at(&horizontal, 50.0) - 20.0).abs() < 0.5);
}

#[test]
fn a_square_nib_has_flat_ends_and_corner_reach_when_turned() {
    let curve = PressureCurve::default();
    let square = Nib::new(NibShape::Square, 0.0, 100.0);
    let path =
        outline_with(&run((10.0, 10.0), (60.0, 10.0)), 10.0, curve, 0.5, square).expect("stroke");
    let xs: Vec<f64> = points(&path).iter().map(|p| p.x).collect();
    let lo = xs.iter().copied().fold(f64::MAX, f64::min);
    let hi = xs.iter().copied().fold(f64::MIN, f64::max);
    assert!(
        (lo - 5.0).abs() < 0.01 && (hi - 65.0).abs() < 0.01,
        "{lo}..{hi}"
    );
    assert!((height(&path) - 10.0).abs() < 0.01);
    // Turned a quarter-eighth, the corner points straight up: half a side times the root of two.
    let diamond = Nib::new(NibShape::Square, 45.0, 100.0);
    let path =
        outline_with(&run((10.0, 10.0), (60.0, 10.0)), 10.0, curve, 0.5, diamond).expect("stroke");
    assert!((height(&path) - 10.0 * 2f64.sqrt()).abs() < 0.01);
}

#[test]
fn nib_settings_are_kept_in_range_and_a_bad_number_means_the_plain_setting() {
    let wrapped = Nib::new(NibShape::Round, -90.0, 250.0);
    assert!((wrapped.angle_degrees() - 270.0).abs() < 1e-9);
    assert!((wrapped.flatness_percent() - 100.0).abs() < 1e-9);
    let thin = Nib::new(NibShape::Square, 400.0, -5.0);
    assert!((thin.angle_degrees() - 40.0).abs() < 1e-9);
    assert!((thin.flatness_percent() - 1.0).abs() < 1e-9);
    let bad = Nib::new(NibShape::Round, f64::NAN, f64::INFINITY);
    assert_eq!(bad, Nib::ROUND);
}

#[test]
fn a_nib_never_makes_a_stroke_that_is_not_a_valid_closed_path() {
    let curve = PressureCurve::new(0.1, 1.0);
    let samples: Vec<_> = (0..=40)
        .map(|i| {
            let t = f64::from(i);
            sample(
                t * 3.0,
                (t * 0.4).sin() * 20.0,
                0.2 + (t % 7.0) as f32 / 9.0,
            )
        })
        .collect();
    for shape in [NibShape::Round, NibShape::Square] {
        for angle in [0.0, 30.0, 90.0, 200.0] {
            for flat in [1.0, 25.0, 100.0] {
                let path = outline_with(&samples, 14.0, curve, 0.5, Nib::new(shape, angle, flat))
                    .expect("outline");
                assert!(matches!(path.first(), Some(PathSegment::MoveTo(_))));
                assert!(matches!(path.last(), Some(PathSegment::Close)));
            }
        }
    }
}

#[test]
fn the_outline_points_of_a_nib_are_its_edge() {
    let round = Nib::ROUND.outline_points(10.0, 32);
    assert_eq!(round.len(), 32);
    assert!(round.iter().all(|p| (p.length() - 10.0).abs() < 1e-9));
    // A flat nib is squashed across its long axis and still reaches the full length along it.
    let flat = Nib::new(NibShape::Round, 0.0, 20.0).outline_points(10.0, 64);
    let tall = flat.iter().map(|p| p.y.abs()).fold(0.0, f64::max);
    let wide = flat.iter().map(|p| p.x.abs()).fold(0.0, f64::max);
    assert!(
        (wide - 10.0).abs() < 1e-6 && (tall - 2.0).abs() < 0.05,
        "{wide} {tall}"
    );
    // The count is kept to something drawable.
    assert_eq!(Nib::ROUND.outline_points(1.0, 0).len(), 8);
    assert_eq!(Nib::ROUND.outline_points(1.0, 1_000_000).len(), 256);
}

fn crosses(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> bool {
    let side = |p: DVec2, q: DVec2, r: DVec2| (q - p).perp_dot(r - p);
    side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0
}

/// Pairs of outline edges close along the path that cross: slivers in the drawn stroke.
fn local_crossings(path: &[PathSegment]) -> usize {
    let pts = points(path);
    let mut found = 0;
    for i in 0..pts.len().saturating_sub(1) {
        for j in i + 2..(i + 40).min(pts.len().saturating_sub(1)) {
            if crosses(pts[i], pts[i + 1], pts[j], pts[j + 1]) {
                found += 1;
            }
        }
    }
    found
}

#[test]
fn an_unsmoothed_shaky_hand_does_not_make_the_outline_cross_itself() {
    // Points under a pixel apart, each off the line by a little: what a mouse gives with
    // smoothing at nothing.
    let samples: Vec<_> = (0..600)
        .map(|i| {
            let t = f64::from(i);
            let x_noise = (f64::from((i * 7919) % 11) / 11.0 - 0.5) * 0.9;
            let y_noise = (f64::from((i * 104_729) % 13) / 13.0 - 0.5) * 0.9;
            sample(t * 0.7 + x_noise, (t * 0.04).sin() * 50.0 + y_noise, 1.0)
        })
        .collect();
    let path = outline(&samples, 12.0, PressureCurve::new(0.15, 1.0), 0.0).expect("outline");
    assert!(
        local_crossings(&path) <= 12,
        "{} slivers",
        local_crossings(&path)
    );
}

#[test]
fn a_tight_turn_is_still_followed_and_a_dot_of_movement_still_draws() {
    let corner = [
        sample(0.0, 0.0, 1.0),
        sample(30.0, 0.0, 1.0),
        sample(30.0, 30.0, 1.0),
    ];
    let path = outline(&corner, 6.0, PressureCurve::default(), 0.0).expect("outline");
    let xs: Vec<f64> = points(&path).iter().map(|p| p.x).collect();
    let ys: Vec<f64> = points(&path).iter().map(|p| p.y).collect();
    let reach = |v: &[f64]| {
        (
            v.iter().copied().fold(f64::MAX, f64::min),
            v.iter().copied().fold(f64::MIN, f64::max),
        )
    };
    let (x_lo, x_hi) = reach(&xs);
    let (y_lo, y_hi) = reach(&ys);
    assert!(x_lo <= -2.9 && x_hi >= 32.9, "{x_lo}..{x_hi}");
    assert!(y_lo <= -2.9 && y_hi >= 32.9, "{y_lo}..{y_hi}");
    let tiny = [sample(5.0, 5.0, 1.0), sample(5.3, 5.0, 1.0)];
    assert!(outline(&tiny, 8.0, PressureCurve::default(), 0.0).is_some());
}
