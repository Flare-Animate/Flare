#![allow(clippy::expect_used)]

use flare_input::PointerKind;

use super::*;

fn pen(x: f64, y: f64, pressure: f32) -> InputSample {
    InputSample::new(x, y, pressure, PointerKind::Pen).expect("sample")
}

fn mouse_run(length: f64, step: f64) -> Vec<InputSample> {
    let count = (length / step) as usize;
    (0..=count)
        .map(|i| InputSample::mouse(i as f64 * step, 0.0).expect("sample"))
        .collect()
}

#[test]
fn a_taper_eases_in_and_out_along_a_quarter_sine() {
    let taper = Taper::new(20.0, 40.0);
    assert_eq!(taper.factor(0.0, 200.0), 0.0);
    assert!((taper.factor(10.0, 200.0) - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
    assert_eq!(taper.factor(20.0, 200.0), 1.0);
    assert_eq!(taper.factor(100.0, 200.0), 1.0);
    assert!((taper.factor(180.0, 200.0) - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-12);
    assert!(taper.factor(200.0, 200.0).abs() < 1e-12);
    for at in [0.0, 25.0, 100.0, 170.0, 200.0] {
        assert_eq!(Taper::NONE.factor(at, 200.0), 1.0);
    }
}

#[test]
fn a_stroke_shorter_than_its_tapers_tapers_over_all_of_itself() {
    // 20 and 20 asked for, 20 long: each end gets 10, so the middle is still the full pressure.
    let taper = Taper::new(20.0, 20.0);
    assert!((taper.factor(10.0, 20.0) - 1.0).abs() < 1e-12);
    assert!(taper.factor(5.0, 20.0) < 1.0);
    assert!(taper.factor(0.0, 20.0).abs() < 1e-12);
    assert_eq!(taper.factor(0.0, 0.0), 1.0, "a dot has nothing to taper");
}

#[test]
fn taper_lengths_are_kept_in_range() {
    let wild = Taper::new(f64::NAN, -5.0);
    assert_eq!((wild.start(), wild.end()), (0.0, 0.0));
    let long = Taper::new(1.0e9, f64::INFINITY);
    assert_eq!((long.start(), long.end()), (MAX_TAPER, 0.0));
}

#[test]
fn the_spine_keeps_the_first_sample_at_each_place() {
    let samples = [
        pen(0.0, 0.0, 0.2),
        pen(0.0, 0.0, 0.9),
        pen(3.0, 0.0, 0.5),
        pen(3.0, 0.0, 0.1),
    ];
    let spine = BrushStroke::spine(&samples);
    assert_eq!(spine.len(), 2);
    assert_eq!((spine[0].pressure, spine[1].pressure), (0.2, 0.5));
}

#[test]
fn a_tapered_mouse_stroke_thins_to_the_lightest_touch_at_both_ends() {
    let stroke = BrushStroke {
        taper: Taper::new(20.0, 20.0),
        ..BrushStroke::new(10.0, PressureCurve::new(0.1, 1.0))
    };
    let spine = BrushStroke::spine(&mouse_run(100.0, 1.0));
    let factors = stroke.width_factors(&spine);
    assert!((factors[0] - 0.1).abs() < 1e-6);
    assert!((factors[factors.len() - 1] - 0.1).abs() < 1e-6);
    assert!((factors[50] - 1.0).abs() < 1e-6);
    // It widens all the way through the taper.
    assert!(factors[..20].windows(2).all(|w| w[0] <= w[1]));
}

#[test]
fn a_taper_to_nothing_closes_the_outline_on_the_first_and_last_points() {
    let samples = mouse_run(100.0, 2.0);
    let stroke = BrushStroke {
        taper: Taper::new(30.0, 30.0),
        ..BrushStroke::new(10.0, PressureCurve::new(0.0, 1.0))
    };
    let path = stroke.outline(&samples, 0.5).expect("outline");
    let points: Vec<DVec2> = path
        .iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some(p.as_vec()),
            _ => None,
        })
        .collect();
    let left = points.iter().map(|p| p.x).fold(f64::MAX, f64::min);
    let right = points.iter().map(|p| p.x).fold(f64::MIN, f64::max);
    assert!(
        left > -1e-9 && right < 100.0 + 1e-9,
        "no round end sticks out: {left}..{right}"
    );
    let widest = points.iter().map(|p| p.y.abs()).fold(0.0, f64::max);
    assert!(
        (widest - 5.0).abs() < 0.1,
        "the middle is full width: {widest}"
    );
    // The same stroke without its taper reaches past both ends by its half width.
    let blunt = BrushStroke {
        taper: Taper::NONE,
        ..stroke
    };
    let path = blunt.outline(&samples, 0.5).expect("outline");
    let left = path
        .iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some(p.as_vec().x),
            _ => None,
        })
        .fold(f64::MAX, f64::min);
    assert!(left < -4.0);
}

#[test]
fn the_preview_halves_are_the_outlines_widths() {
    let samples: Vec<_> = (0..=30)
        .map(|i| pen(f64::from(i) * 3.0, 0.0, i as f32 / 30.0))
        .collect();
    let stroke = BrushStroke {
        velocity: 0.4,
        taper: Taper::new(10.0, 10.0),
        ..BrushStroke::new(12.0, PressureCurve::new(0.2, 1.3))
    };
    let spine = BrushStroke::spine(&samples);
    let halves = stroke.half_widths(&spine);
    let factors = stroke.width_factors(&spine);
    for (half, factor) in halves.iter().zip(&factors) {
        assert!(
            (half - 6.0 * factor).abs() < 1e-9,
            "a round nib reaches fully across"
        );
    }
    // A flat nib running along its long axis reaches across less.
    let flat = BrushStroke {
        nib: Nib::new(NibShape::Round, 0.0, 20.0),
        ..stroke
    };
    let flat_halves = flat.half_widths(&spine);
    assert!(flat_halves[15] < halves[15] * 0.5);
}

#[test]
fn velocity_and_pressure_are_blended_as_before() {
    // The rule the outline always used, written out: half way between pressure and speed.
    let curve = PressureCurve::new(0.2, 1.0);
    let stroke = BrushStroke {
        velocity: 0.5,
        ..BrushStroke::new(10.0, curve.clone())
    };
    let spine = [pen(0.0, 0.0, 1.0), pen(24.0, 0.0, 0.5), pen(48.0, 0.0, 1.0)];
    let factors = stroke.width_factors(&spine);
    let pressure = f64::from(curve.width_factor(0.5));
    let speed = 1.0 - 0.8 * 1.0;
    assert!((factors[1] - (pressure + (speed - pressure) * 0.5)).abs() < 1e-6);
}

#[test]
fn uneven_steps_do_not_loop_the_line_back_past_its_start() {
    // A stabilizer starts a stroke with tiny steps and then takes long ones; a uniform spline
    // swung back half a unit behind the first point there.
    let xs = [0.0, 0.33, 1.0, 2.0, 3.33, 5.0, 25.0, 45.0, 65.0];
    let samples: Vec<InputSample> = xs.iter().map(|x| pen(*x, 0.0, 1.0)).collect();
    let stroke = BrushStroke {
        taper: Taper::new(20.0, 20.0),
        ..BrushStroke::new(10.0, PressureCurve::new(0.0, 1.0))
    };
    let path = stroke.outline(&samples, 2.0).expect("outline");
    let left = path
        .iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) => Some(p.as_vec().x),
            _ => None,
        })
        .fold(f64::MAX, f64::min);
    assert!(left > -1e-9, "{left}");
}
