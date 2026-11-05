#![allow(clippy::expect_used, clippy::unwrap_used)]
//! Failure paths: sizes, lengths, mismatches, settings and times that must be refused.

use super::*;

#[test]
fn a_raster_needs_a_size_within_bounds() {
    assert_eq!(Raster::new(0, 4, &[]).unwrap_err(), RasterError::Size);
    assert_eq!(Raster::new(4, 0, &[]).unwrap_err(), RasterError::Size);
    let wide = vec![0u8; ((MAX_SIDE + 1) * 4) as usize];
    assert_eq!(
        Raster::new(MAX_SIDE + 1, 1, &wide).unwrap_err(),
        RasterError::Size
    );
}

#[test]
fn a_raster_is_limited_in_pixels() {
    let pixels = vec![0u8; (MAX_SIDE as usize) * 2049 * 4];
    let err = Raster::new(MAX_SIDE, 2049, &pixels).unwrap_err();
    assert_eq!(err, RasterError::TooManyPixels);
}

#[test]
fn a_raster_needs_exactly_its_bytes() {
    let err = Raster::new(2, 2, &[0; 15]).unwrap_err();
    assert_eq!(
        err,
        RasterError::Length {
            width: 2,
            height: 2,
            expected: 16,
            got: 15
        }
    );
}

#[test]
fn two_pictures_must_have_the_same_size() {
    let a_pixels = vec![0u8; 4 * 4 * 4];
    let b_pixels = vec![0u8; 4 * 5 * 4];
    let a = Raster::new(4, 4, &a_pixels).expect("raster");
    let b = Raster::new(4, 5, &b_pixels).expect("raster");
    let err = Inbetweener::new(a, b, FlowParams::default()).unwrap_err();
    assert!(matches!(
        err,
        RasterError::Mismatch {
            a_height: 4,
            b_height: 5,
            ..
        }
    ));
}

#[test]
fn flow_settings_are_bounded() {
    let pixels = vec![0u8; 4 * 4 * 4];
    let a = Raster::new(4, 4, &pixels).expect("raster");
    let cases = [
        (
            FlowParams {
                levels: 0,
                ..FlowParams::default()
            },
            "levels",
        ),
        (
            FlowParams {
                levels: MAX_LEVELS + 1,
                ..FlowParams::default()
            },
            "levels",
        ),
        (
            FlowParams {
                iterations: 0,
                ..FlowParams::default()
            },
            "iterations",
        ),
        (
            FlowParams {
                iterations: MAX_ITERATIONS + 1,
                ..FlowParams::default()
            },
            "iterations",
        ),
        (
            FlowParams {
                window: 0,
                ..FlowParams::default()
            },
            "window",
        ),
        (
            FlowParams {
                window: MAX_WINDOW + 1,
                ..FlowParams::default()
            },
            "window",
        ),
    ];
    for (params, name) in cases {
        match Inbetweener::new(a, a, params) {
            Err(RasterError::Param { name: got, .. }) => assert_eq!(got, name),
            other => panic!("{params:?} gave {other:?}"),
        }
    }
    let widest = FlowParams {
        levels: MAX_LEVELS,
        iterations: MAX_ITERATIONS,
        window: MAX_WINDOW,
    };
    assert!(
        Inbetweener::new(a, a, widest).is_ok(),
        "the bounds themselves are allowed"
    );
}

#[test]
fn a_time_outside_zero_to_one_is_refused() {
    let pixels = vec![7u8; 2 * 2 * 4];
    let a = Raster::new(2, 2, &pixels).expect("raster");
    let inbetweener = Inbetweener::new(a, a, FlowParams::default()).expect("flow");
    for t in [-0.1, 1.01, f64::NAN, f64::INFINITY] {
        assert!(
            matches!(inbetweener.frame(t), Err(RasterError::Time(_))),
            "t = {t} should be refused"
        );
    }
    assert!(inbetweener.frame(0.0).is_ok());
    assert!(inbetweener.frame(1.0).is_ok());
}
