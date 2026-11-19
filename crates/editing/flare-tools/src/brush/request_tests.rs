#![allow(clippy::expect_used)]

use flare_core::Rgba8;

use super::*;

fn request() -> BrushRequest {
    BrushRequest {
        profile: None,
        size: 12.0,
        color: Rgba8::opaque(10, 20, 30),
        flow: None,
        stabilizer: None,
        taper: None,
        smoothing: None,
    }
}

fn range_error(request: BrushRequest) -> &'static str {
    match request.resolve() {
        Err(RequestError::OutOfRange { setting, .. }) => setting,
        other => panic!("expected a range error, got {other:?}"),
    }
}

#[test]
fn the_plain_brush_and_the_desktop_smoothing_are_the_start() {
    let (brush, paint) = request().resolve().expect("valid");
    assert_eq!(brush, BuiltinBrushProfile::Plain.preset().stroke(12.0));
    assert_eq!(paint.tolerance, DEFAULT_SMOOTHING);
    assert_eq!(paint.flow, 1.0);
    assert_eq!(paint.color, Rgba8::opaque(10, 20, 30));
}

#[test]
fn a_profile_is_found_by_name_and_its_settings_are_kept() {
    let (brush, paint) = BrushRequest {
        profile: Some("G-pen".to_owned()),
        ..request()
    }
    .resolve()
    .expect("valid");
    let preset = BuiltinBrushProfile::GPen.preset();
    assert_eq!(brush, preset.stroke(12.0));
    assert_eq!(paint.stabilizer, preset.stabilizer());
}

#[test]
fn settings_override_the_profile() {
    let (brush, paint) = BrushRequest {
        profile: Some("ink".to_owned()),
        flow: Some(50.0),
        stabilizer: Some(6),
        taper: Some((20.0, 30.0)),
        smoothing: Some(0.5),
        ..request()
    }
    .resolve()
    .expect("valid");
    assert_eq!(brush.taper, Taper::new(20.0, 30.0));
    assert_eq!(paint.flow, 0.5);
    assert_eq!(paint.stabilizer, 6);
    assert_eq!(paint.tolerance, 0.5);
}

#[test]
fn an_unknown_profile_names_the_ones_there_are() {
    let error = BrushRequest {
        profile: Some("airbrush".to_owned()),
        ..request()
    }
    .resolve()
    .expect_err("unknown");
    assert_eq!(error, RequestError::UnknownProfile("airbrush".to_owned()));
    assert!(error.to_string().contains("g-pen"));
}

#[test]
fn settings_out_of_range_are_refused() {
    assert_eq!(
        range_error(BrushRequest {
            size: 0.5,
            ..request()
        }),
        "size"
    );
    assert_eq!(
        range_error(BrushRequest {
            size: f64::NAN,
            ..request()
        }),
        "size"
    );
    assert_eq!(
        range_error(BrushRequest {
            flow: Some(101.0),
            ..request()
        }),
        "flow"
    );
    assert_eq!(
        range_error(BrushRequest {
            stabilizer: Some(flare_input::MAX_STABILIZER_LEVEL + 1),
            ..request()
        }),
        "stabilizer"
    );
    assert_eq!(
        range_error(BrushRequest {
            taper: Some((-1.0, 0.0)),
            ..request()
        }),
        "taper"
    );
    assert_eq!(
        range_error(BrushRequest {
            smoothing: Some(MAX_SMOOTHING + 1.0),
            ..request()
        }),
        "smoothing"
    );
}

#[test]
fn points_take_an_optional_pressure() {
    let samples = stroke_samples(0, &[vec![1.0, 2.0], vec![3.0, 4.0, 0.25]]).expect("valid");
    assert_eq!(
        (samples[0].x, samples[0].y, samples[0].pressure),
        (1.0, 2.0, 1.0)
    );
    assert_eq!(
        (samples[1].x, samples[1].y, samples[1].pressure),
        (3.0, 4.0, 0.25)
    );
}

#[test]
fn bad_points_are_named() {
    assert_eq!(
        stroke_samples(2, &[vec![1.0, 2.0], vec![1.0]]),
        Err(RequestError::PointShape {
            stroke: 2,
            point: 1
        })
    );
    assert_eq!(
        stroke_samples(0, &[vec![1.0, 2.0, 3.0, 4.0]]),
        Err(RequestError::PointShape {
            stroke: 0,
            point: 0
        })
    );
    assert_eq!(
        stroke_samples(1, &[vec![f64::INFINITY, 2.0]]),
        Err(RequestError::NotANumber {
            stroke: 1,
            point: 0
        })
    );
    assert_eq!(
        stroke_samples(0, &[vec![1.0, 2.0, 1.5]]),
        Err(RequestError::Pressure {
            stroke: 0,
            point: 0
        })
    );
}
