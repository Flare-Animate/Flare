#![allow(clippy::expect_used)]
use std::time::{Duration, Instant};

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
    assert_eq!(PenFeed::default().reading(Instant::now()), None);
}

#[test]
fn a_pen_on_the_surface_keeps_its_pressure_however_still_it_is() {
    let feed = PenFeed::default();
    let start = Instant::now();
    feed.press(0.6, start);
    assert_eq!(feed.reading(start + Duration::from_secs(30)), Some(0.6));
}

#[test]
fn a_hovering_pen_counts_only_while_it_keeps_reporting() {
    let feed = PenFeed::default();
    let start = Instant::now();
    feed.hover(0.0, start);
    assert_eq!(feed.reading(start + Duration::from_millis(100)), Some(0.0));
    assert_eq!(
        feed.reading(start + Duration::from_secs(2)),
        None,
        "a mouse used later must not read as a pen at no pressure"
    );
}

#[test]
fn lifting_the_pen_hands_the_stage_back_to_the_mouse_at_once() {
    let feed = PenFeed::default();
    let start = Instant::now();
    feed.press(0.9, start);
    feed.lift();
    assert_eq!(feed.reading(start), None);
}

#[test]
fn a_clone_sees_what_the_other_writes_and_pressure_is_clamped() {
    let feed = PenFeed::default();
    let reader = feed.clone();
    let start = Instant::now();
    feed.press(7.0, start);
    assert_eq!(reader.reading(start), Some(1.0));
}
