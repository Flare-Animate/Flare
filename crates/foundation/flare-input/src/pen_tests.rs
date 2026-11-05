#![allow(clippy::expect_used)]

use crate::pen::*;

#[test]
fn an_axis_maps_any_range_to_zero_to_one_and_clamps() {
    let axis = PressureAxis::new(0.0, 2047.0).expect("axis");
    assert_eq!(axis.normalize(0.0), 0.0);
    assert_eq!(axis.normalize(2047.0), 1.0);
    assert!((axis.normalize(1023.5) - 0.5).abs() < 1e-6);
    assert_eq!(axis.normalize(-5.0), 0.0);
    assert_eq!(axis.normalize(9000.0), 1.0);
    let offset = PressureAxis::new(100.0, 300.0).expect("axis");
    assert!((offset.normalize(200.0) - 0.5).abs() < 1e-6);
}

#[test]
fn a_reading_that_is_not_a_number_is_full_pressure_not_a_thin_line() {
    let axis = PressureAxis::new(0.0, 100.0).expect("axis");
    assert_eq!(axis.normalize(f64::NAN), 1.0);
    assert_eq!(axis.normalize(f64::INFINITY), 1.0);
    assert_eq!(axis.normalize(f64::NEG_INFINITY), 0.0);
}

#[test]
fn an_axis_that_is_not_a_range_is_refused() {
    assert!(
        PressureAxis::new(0.0, 0.0).is_none(),
        "a device with no pressure"
    );
    assert!(PressureAxis::new(5.0, 1.0).is_none());
    assert!(PressureAxis::new(f64::NAN, 1.0).is_none());
    assert!(PressureAxis::new(0.0, f64::INFINITY).is_none());
}

#[test]
fn a_feed_nobody_wrote_to_is_no_pen() {
    assert_eq!(PenFeed::default().reading(), None);
}

#[test]
fn the_pressure_set_is_read_back_clamped_until_the_pen_stops_being_the_device() {
    let feed = PenFeed::default();
    let reader = feed.clone();
    feed.set(Some(0.3));
    assert_eq!(
        reader.reading(),
        Some(0.3),
        "a clone sees what the other writes"
    );
    feed.set(Some(7.0));
    assert_eq!(reader.reading(), Some(1.0));
    feed.set(Some(f32::NAN));
    assert_eq!(reader.reading(), Some(1.0));
    feed.set(None);
    assert_eq!(reader.reading(), None);
}

#[test]
fn only_a_pen_reports_pressure_and_every_other_device_is_no_pen() {
    assert_eq!(pen_pressure("mouse", 0.2), None);
    assert_eq!(pen_pressure("touch", 0.7), None);
    assert_eq!(pen_pressure("", 0.5), None, "an event with no pointer type");
    assert_eq!(
        pen_pressure("PEN", 0.5),
        None,
        "pointerType values are lowercase"
    );
    assert_eq!(pen_pressure("pen", 0.25), Some(0.25));
}

#[test]
fn a_pen_pressure_that_is_out_of_range_or_missing_is_clamped_not_trusted() {
    assert_eq!(pen_pressure("pen", 3.0), Some(1.0));
    assert_eq!(pen_pressure("pen", -0.5), Some(0.0));
    assert_eq!(pen_pressure("pen", f64::INFINITY), Some(1.0));
    assert_eq!(pen_pressure("pen", f64::NEG_INFINITY), Some(0.0));
    assert_eq!(pen_pressure("pen", f64::NAN), Some(1.0));
    assert_eq!(pen_pressure("pen", 0.0), Some(0.0), "a hovering pen");
}
