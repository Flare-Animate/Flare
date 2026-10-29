#![allow(clippy::expect_used)]

use super::*;

#[test]
fn samples_reject_non_finite_positions_and_clamp_pressure() {
    assert!(InputSample::new(f64::NAN, 0.0, 1.0, PointerKind::Pen).is_none());
    assert!(InputSample::new(0.0, f64::INFINITY, 1.0, PointerKind::Pen).is_none());
    let hard = InputSample::new(1.0, 2.0, 3.0, PointerKind::Pen).expect("sample");
    assert_eq!(hard.pressure, 1.0);
    let negative = InputSample::new(1.0, 2.0, -1.0, PointerKind::Pen).expect("sample");
    assert_eq!(negative.pressure, 0.0);
    let unknown = InputSample::new(1.0, 2.0, f32::NAN, PointerKind::Touch).expect("sample");
    assert_eq!(
        unknown.pressure, 1.0,
        "no reading means no reason to thin the line"
    );
}

#[test]
fn a_mouse_is_full_pressure() {
    let sample = InputSample::mouse(3.0, 4.0).expect("sample");
    assert_eq!((sample.pressure, sample.kind), (1.0, PointerKind::Mouse));
}

#[test]
fn the_curve_runs_from_its_minimum_to_full_width_and_bends_with_gamma() {
    let linear = PressureCurve::new(0.2, 1.0);
    assert_eq!(linear.width_factor(0.0), 0.2);
    assert_eq!(linear.width_factor(1.0), 1.0);
    assert!((linear.width_factor(0.5) - 0.6).abs() < 1e-6);
    let light = PressureCurve::new(0.0, 0.5);
    let firm = PressureCurve::new(0.0, 2.0);
    assert!(light.width_factor(0.5) > 0.5 && firm.width_factor(0.5) < 0.5);
    // Nonsense settings still give widths between the minimum and one.
    let odd = PressureCurve::new(f32::NAN, -3.0);
    assert_eq!(odd.width_factor(0.7), 0.7);
    assert_eq!(linear.width_factor(9.0), 1.0, "pressure is clamped");
}
