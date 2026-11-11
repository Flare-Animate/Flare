#![allow(clippy::expect_used)]

use super::*;

fn pen(x: f64, y: f64, pressure: f32) -> InputSample {
    InputSample::new(x, y, pressure, PointerKind::Pen).expect("sample")
}

#[test]
fn level_zero_or_one_hands_samples_through_and_has_nothing_to_catch_up() {
    for level in [0, 1] {
        let mut steady = Stabilizer::new(level, true, 1.0);
        for i in 0..5 {
            let s = pen(f64::from(i) * 7.0, 3.0, 0.4);
            assert_eq!(steady.push(s), s);
        }
        assert!(steady.finish().is_empty());
    }
}

#[test]
fn a_jump_is_eased_in_over_the_window_starting_where_the_pen_went_down() {
    let mut steady = Stabilizer::new(6, false, 1.0);
    let mut xs = vec![steady.push(pen(0.0, 0.0, 1.0)).x];
    for _ in 0..6 {
        xs.push(steady.push(pen(60.0, 0.0, 1.0)).x);
    }
    let expected = [0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0];
    for (got, want) in xs.iter().zip(expected) {
        assert!((got - want).abs() < 1e-9, "{xs:?}");
    }
}

#[test]
fn pressure_is_averaged_with_the_position() {
    let mut steady = Stabilizer::new(4, false, 1.0);
    steady.push(pen(0.0, 0.0, 0.0));
    steady.push(pen(1.0, 0.0, 1.0));
    steady.push(pen(2.0, 0.0, 0.0));
    let out = steady.push(pen(3.0, 0.0, 1.0));
    assert!((out.pressure - 0.5).abs() < 1e-6);
    assert!((out.x - 1.5).abs() < 1e-9);
}

#[test]
fn hand_shake_is_smoothed_out() {
    let mut steady = Stabilizer::new(6, false, 1.0);
    let mut worst = 0.0_f64;
    for i in 0..200 {
        let wobble = if i % 2 == 0 { 3.0 } else { -3.0 };
        let out = steady.push(pen(f64::from(i), wobble, 1.0));
        if i > 6 {
            worst = worst.max(out.y.abs());
        }
    }
    assert!(worst < 1.0, "a 3 px shake stays {worst} px off the line");
}

#[test]
fn finishing_brings_the_line_to_where_the_pen_was_let_go() {
    let mut steady = Stabilizer::new(10, true, 1.0);
    let mut last = pen(0.0, 0.0, 1.0);
    for i in 0..30 {
        last = steady.push(pen(f64::from(i) * 4.0, f64::from(i), 0.7));
    }
    assert!(last.x < 116.0, "the line trails the pen while it moves");
    let tail = steady.finish();
    let end = tail.last().expect("a catch-up");
    assert_eq!((end.x, end.y), (116.0, 29.0));
    assert!((end.pressure - 0.7).abs() < 1e-6);
    let mut fresh = Stabilizer::new(10, true, 1.0);
    assert!(
        fresh.finish().is_empty(),
        "nothing drawn, nothing to catch up"
    );
}

#[test]
fn a_fast_move_lags_less_when_the_window_follows_speed() {
    let lag = |by_speed: bool| {
        let mut steady = Stabilizer::new(12, by_speed, 1.0);
        let mut out = pen(0.0, 0.0, 1.0);
        for i in 0..20 {
            out = steady.push(pen(f64::from(i) * 300.0, 0.0, 1.0));
        }
        19.0 * 300.0 - out.x
    };
    let (fixed, by_speed) = (lag(false), lag(true));
    assert!(by_speed < fixed / 3.0, "{by_speed} against {fixed}");
    // The same move seen at a quarter of the zoom is a quarter as fast on screen.
    let mut zoomed_out = Stabilizer::new(12, true, 0.25);
    let mut out = pen(0.0, 0.0, 1.0);
    for i in 0..20 {
        out = zoomed_out.push(pen(f64::from(i) * 300.0, 0.0, 1.0));
    }
    assert!(19.0 * 300.0 - out.x > by_speed);
}

#[test]
fn the_level_and_zoom_are_kept_sensible() {
    assert_eq!(
        Stabilizer::new(200, true, 1.0).level(),
        MAX_STABILIZER_LEVEL
    );
    for zoom in [f64::NAN, 0.0, -2.0, f64::INFINITY] {
        let mut steady = Stabilizer::new(8, true, zoom);
        let out = steady.push(pen(5.0, 5.0, 1.0));
        assert_eq!((out.x, out.y), (5.0, 5.0));
        for i in 0..100 {
            let out = steady.push(pen(f64::from(i), 0.0, 1.0));
            assert!(out.x.is_finite() && out.y.is_finite());
        }
    }
}
