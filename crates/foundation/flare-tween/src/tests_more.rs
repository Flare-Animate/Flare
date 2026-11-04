//! More interpolation tests; the helpers are in `tests.rs`.
use super::tests::{near, p};
use crate::{
    Curve, Easing, Extrapolation, HandleKind, Interpolation, Key, MAX_POINTS, Params,
    SequenceOptions, StrokePoint, TweenError, interpolate_sequence,
};

#[test]
fn single_key_holds_everywhere() {
    let curve = Curve::new(vec![Key::new(3.0, 7.0)], Extrapolation::Cyclic, p())
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(curve.evaluate(-100.0).ok(), Some(7.0));
    assert_eq!(curve.evaluate(3.0).ok(), Some(7.0));
}

#[test]
fn auto_handles_are_smooth_and_clamped_ones_are_flat_at_a_peak() {
    let auto = |kind| {
        Curve::new(
            vec![
                Key {
                    handle_kind: kind,
                    ..Key::new(0.0, 0.0)
                },
                Key {
                    handle_kind: kind,
                    ..Key::new(1.0, 1.0)
                },
                Key {
                    handle_kind: kind,
                    ..Key::new(2.0, 0.0)
                },
            ],
            Extrapolation::Constant,
            p(),
        )
        .unwrap_or_else(|e| panic!("{e}"))
    };
    let smooth = auto(HandleKind::Auto);
    let mid = smooth.keys()[1];
    assert!(
        near(mid.left.dv / mid.left.dt, mid.right.dv / mid.right.dt),
        "tangent continuity"
    );
    assert!(near(mid.right.dv, 0.0), "the peak's tangent is level");
    let clamped = auto(HandleKind::AutoClamped);
    assert_eq!(clamped.keys()[1].right.dv, 0.0);
    assert!(clamped.keys()[0].right.dv.abs() < 1e-12, "ends are flat");
}

#[test]
fn vector_handles_point_at_the_neighbour_and_aligned_stay_on_a_line() {
    let curve = Curve::new(
        vec![
            Key {
                handle_kind: HandleKind::Vector,
                ..Key::new(0.0, 0.0)
            },
            Key {
                handle_kind: HandleKind::Aligned,
                right: crate::Handle { dt: 1.0, dv: 2.0 },
                ..Key::new(3.0, 6.0)
            },
            Key::new(6.0, 0.0),
        ],
        Extrapolation::Constant,
        p(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let first = curve.keys()[0];
    assert!(near(first.right.dt, 1.0) && near(first.right.dv, 2.0));
    let mid = curve.keys()[1];
    assert!(
        near(mid.left.dt * mid.right.dv, mid.left.dv * mid.right.dt),
        "aligned: one line"
    );
    assert!(mid.left.dt <= 0.0, "left handle is on the left");
}

#[test]
fn handles_are_clamped_inside_their_segment() {
    let curve = Curve::new(
        vec![
            Key {
                handle_kind: HandleKind::Free,
                right: crate::Handle { dt: 50.0, dv: 1.0 },
                ..Key::new(0.0, 0.0)
            },
            Key::new(2.0, 0.0),
        ],
        Extrapolation::Constant,
        p(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(near(curve.keys()[0].right.dt, 2.0));
}

#[test]
fn bezier_segment_follows_its_handles() {
    let curve = Curve::new(
        vec![
            Key {
                interpolation: Interpolation::Bezier,
                handle_kind: HandleKind::Free,
                ..Key::new(0.0, 0.0)
            },
            Key {
                handle_kind: HandleKind::Free,
                ..Key::new(1.0, 1.0)
            },
        ],
        Extrapolation::Constant,
        p(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mid = curve.evaluate(0.5).unwrap_or(f64::NAN);
    assert!(
        (mid - 0.5).abs() < 1e-9,
        "zero handles on a straight line: {mid}"
    );
}

fn pt(x: f64, y: f64) -> StrokePoint {
    StrokePoint { x, y }
}

#[test]
fn sequence_same_count_lerps_with_easing() {
    let a = [pt(0.0, 0.0), pt(10.0, 0.0)];
    let b = [pt(2.0, 2.0), pt(10.0, 10.0)];
    let mid = interpolate_sequence(
        &a,
        &b,
        &SequenceOptions {
            factor: 0.5,
            ..Default::default()
        },
    )
    .unwrap_or_default();
    assert_eq!(mid, vec![pt(1.0, 1.0), pt(10.0, 5.0)]);
    let eased = interpolate_sequence(
        &a,
        &b,
        &SequenceOptions {
            factor: 0.25,
            interpolation: Interpolation::Quad,
            easing: Easing::In,
            ..Default::default()
        },
    )
    .unwrap_or_default();
    assert!(near(eased[0].x, 2.0 * 0.0625), "quad in at 0.25 is 0.0625");
}

#[test]
fn sequence_resamples_different_counts_by_arc_length() {
    let a = [pt(0.0, 0.0), pt(10.0, 0.0)];
    let b = [pt(0.0, 0.0), pt(1.0, 0.0), pt(10.0, 0.0)];
    let at_start = interpolate_sequence(
        &a,
        &b,
        &SequenceOptions {
            factor: 0.0,
            ..Default::default()
        },
    )
    .unwrap_or_default();
    assert_eq!(at_start, vec![pt(0.0, 0.0), pt(5.0, 0.0), pt(10.0, 0.0)]);
    let at_end = interpolate_sequence(
        &a,
        &b,
        &SequenceOptions {
            factor: 1.0,
            ..Default::default()
        },
    )
    .unwrap_or_default();
    assert_eq!(at_end, b.to_vec());
}

#[test]
fn sequence_flip_reverses_the_second_stroke() {
    let a = [pt(0.0, 0.0), pt(1.0, 0.0)];
    let b = [pt(5.0, 5.0), pt(6.0, 5.0)];
    let flipped = interpolate_sequence(
        &a,
        &b,
        &SequenceOptions {
            factor: 1.0,
            flip: true,
            ..Default::default()
        },
    )
    .unwrap_or_default();
    assert_eq!(flipped, vec![pt(6.0, 5.0), pt(5.0, 5.0)]);
}

#[test]
fn sequence_refuses_bad_input() {
    let one = [pt(0.0, 0.0)];
    let o = SequenceOptions::default();
    assert_eq!(
        interpolate_sequence(&[], &one, &o),
        Err(TweenError::PointCount {
            count: 0,
            max: MAX_POINTS
        })
    );
    let big = vec![pt(0.0, 0.0); MAX_POINTS + 1];
    assert!(matches!(
        interpolate_sequence(&big, &one, &o),
        Err(TweenError::PointCount { .. })
    ));
    assert!(matches!(
        interpolate_sequence(&[pt(f64::NAN, 0.0)], &one, &o),
        Err(TweenError::NotFinite { .. })
    ));
    assert!(matches!(
        interpolate_sequence(&one, &one, &SequenceOptions { factor: 1.5, ..o }),
        Err(TweenError::OutOfRange { .. })
    ));
    assert!(matches!(
        interpolate_sequence(
            &one,
            &one,
            &SequenceOptions {
                factor: f64::NAN,
                ..o
            }
        ),
        Err(TweenError::NotFinite { .. })
    ));
}

#[test]
fn sequence_overflow_is_refused_and_a_still_stroke_stays_put() {
    let a = [pt(-1e308, 0.0)];
    let b = [pt(1e308, 0.0)];
    let at_end = SequenceOptions {
        factor: 1.0,
        ..SequenceOptions::default()
    };
    assert_eq!(
        interpolate_sequence(&a, &b, &at_end).map(|out| out[0].x),
        Ok(1e308)
    );
    // Back Out passes 1 on the way, so the in-between leaves the range of both strokes and overflows.
    let overshoot = SequenceOptions {
        factor: 0.5,
        interpolation: Interpolation::Back,
        easing: Easing::Out,
        params: Params {
            back_overshoot: 10.0,
            ..Params::default()
        },
        ..SequenceOptions::default()
    };
    assert_eq!(
        interpolate_sequence(&a, &b, &overshoot),
        Err(TweenError::NonFiniteResult)
    );
    let still = [pt(3.0, 3.0), pt(3.0, 3.0)];
    let out = interpolate_sequence(
        &still,
        &[pt(3.0, 3.0)],
        &SequenceOptions {
            factor: 0.5,
            ..SequenceOptions::default()
        },
    )
    .unwrap_or_default();
    assert_eq!(out, vec![pt(3.0, 3.0), pt(3.0, 3.0)]);
}
