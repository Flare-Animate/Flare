use crate::bezier::cubic_at;
use crate::{
    Curve, Easing, Extrapolation, HandleKind, Interpolation, Key, MAX_KEYS, MAX_POINTS, Params,
    SequenceOptions, StrokePoint, TweenError, ease, interpolate_sequence,
};

const EPS: f64 = 1e-9;

fn p() -> Params {
    Params::default()
}

fn key(time: f64, value: f64, kind: Interpolation) -> Key {
    Key {
        interpolation: kind,
        ..Key::new(time, value)
    }
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

#[test]
fn names_round_trip() {
    for kind in Interpolation::ALL {
        assert_eq!(Interpolation::parse(kind.name()), Some(kind));
    }
    for easing in Easing::ALL {
        assert_eq!(Easing::parse(easing.name()), Some(easing));
    }
    for mode in Extrapolation::ALL {
        assert_eq!(Extrapolation::parse(mode.name()), Some(mode));
    }
    assert_eq!(Interpolation::parse("wobble"), None);
    assert_eq!(Easing::parse(""), None);
}

#[test]
fn every_family_is_exact_at_both_ends() {
    for kind in Interpolation::ALL {
        for easing in Easing::ALL {
            assert_eq!(
                ease(kind, easing, 0.0, &p()).ok(),
                Some(0.0),
                "{kind:?} {easing:?}"
            );
            assert_eq!(
                ease(kind, easing, 1.0, &p()).ok(),
                Some(1.0),
                "{kind:?} {easing:?}"
            );
        }
    }
}

#[test]
fn monotone_families_do_not_go_back() {
    let monotone = [
        Interpolation::Sine,
        Interpolation::Quad,
        Interpolation::Cubic,
        Interpolation::Quart,
        Interpolation::Quint,
        Interpolation::Expo,
        Interpolation::Circ,
    ];
    for kind in monotone {
        for easing in Easing::ALL {
            let mut last = -1.0;
            for step in 0..=200 {
                let t = f64::from(step) / 200.0;
                let v = ease(kind, easing, t, &p()).unwrap_or(f64::NAN);
                assert!(
                    v >= last - EPS && (-EPS..=1.0 + EPS).contains(&v),
                    "{kind:?} {easing:?} t={t}"
                );
                last = v;
            }
        }
    }
}

#[test]
fn linear_ignores_easing_and_constant_steps() {
    assert!(near(
        ease(Interpolation::Linear, Easing::In, 0.3, &p()).unwrap_or(f64::NAN),
        0.3
    ));
    assert_eq!(
        ease(Interpolation::Constant, Easing::Out, 0.99, &p()).ok(),
        Some(0.0)
    );
    assert_eq!(
        ease(Interpolation::Constant, Easing::Out, 1.0, &p()).ok(),
        Some(1.0)
    );
}

#[test]
fn time_is_clamped_and_non_finite_refused() {
    assert_eq!(
        ease(Interpolation::Quad, Easing::In, -4.0, &p()).ok(),
        Some(0.0)
    );
    assert_eq!(
        ease(Interpolation::Quad, Easing::In, 4.0, &p()).ok(),
        Some(1.0)
    );
    assert_eq!(
        ease(Interpolation::Quad, Easing::In, f64::NAN, &p()),
        Err(TweenError::NotFinite { what: "time" })
    );
    assert!(ease(Interpolation::Quad, Easing::In, f64::INFINITY, &p()).is_err());
}

#[test]
fn bad_parameters_are_refused() {
    let bad = [
        Params {
            back_overshoot: -1.0,
            ..p()
        },
        Params {
            back_overshoot: f64::NAN,
            ..p()
        },
        Params {
            elastic_amplitude: 0.5,
            ..p()
        },
        Params {
            elastic_period: 0.0,
            ..p()
        },
        Params {
            elastic_period: 1e6,
            ..p()
        },
    ];
    for params in bad {
        assert!(
            ease(Interpolation::Back, Easing::Out, 0.5, &params).is_err(),
            "{params:?}"
        );
    }
}

#[test]
fn back_overshoots_and_elastic_wobbles() {
    let back: Vec<f64> = (1..10)
        .map(|i| ease(Interpolation::Back, Easing::Out, f64::from(i) / 10.0, &p()).unwrap_or(0.0))
        .collect();
    assert!(back.iter().any(|v| *v > 1.0), "Back Out must pass 1");
    let elastic_overshoot = (1..100)
        .map(|i| {
            ease(
                Interpolation::Elastic,
                Easing::Out,
                f64::from(i) / 100.0,
                &p(),
            )
            .unwrap_or(0.0)
        })
        .any(|v| v > 1.0);
    assert!(elastic_overshoot);
}

#[test]
fn bezier_cubic_with_linear_handles_is_identity() {
    let xs = [0.0, 1.0 / 3.0, 2.0 / 3.0, 1.0];
    for step in 0..=20 {
        let x = f64::from(step) / 20.0;
        assert!((cubic_at(xs, xs, x) - x).abs() < 1e-12);
    }
}

#[test]
fn curve_hits_its_keys_and_interpolates() {
    let curve = Curve::new(
        vec![
            key(10.0, 0.0, Interpolation::Linear),
            key(0.0, 4.0, Interpolation::Linear),
        ],
        Extrapolation::Constant,
        p(),
    );
    let curve = curve.unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(curve.keys()[0].time, 0.0, "keys are sorted by time");
    assert!(near(curve.evaluate(0.0).unwrap_or(f64::NAN), 4.0));
    assert!(near(curve.evaluate(10.0).unwrap_or(f64::NAN), 0.0));
    assert!(near(curve.evaluate(5.0).unwrap_or(f64::NAN), 2.0));
}

#[test]
fn curve_refuses_bad_input() {
    let ok = |keys: Vec<Key>| Curve::new(keys, Extrapolation::Constant, p());
    assert_eq!(
        ok(vec![]),
        Err(TweenError::KeyCount {
            count: 0,
            max: MAX_KEYS
        })
    );
    let too_many = (0..=MAX_KEYS).map(|i| Key::new(i as f64, 0.0)).collect();
    assert!(matches!(ok(too_many), Err(TweenError::KeyCount { .. })));
    assert_eq!(
        ok(vec![Key::new(1.0, 0.0), Key::new(1.0, 2.0)]),
        Err(TweenError::DuplicateTime { time: 1.0 })
    );
    assert!(matches!(
        ok(vec![Key::new(f64::NAN, 0.0)]),
        Err(TweenError::NotFinite { .. })
    ));
    assert!(matches!(
        ok(vec![Key::new(0.0, f64::INFINITY)]),
        Err(TweenError::NotFinite { .. })
    ));
    let curve = ok(vec![Key::new(0.0, 0.0)]).unwrap_or_else(|e| panic!("{e}"));
    assert!(curve.evaluate(f64::NAN).is_err());
}

#[test]
fn overflowing_result_is_refused_not_infinite() {
    let curve = Curve::new(
        vec![
            key(0.0, -1e308, Interpolation::Linear),
            key(1.0, 1e308, Interpolation::Linear),
        ],
        Extrapolation::Linear,
        p(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(curve.evaluate(1e300), Err(TweenError::NonFiniteResult));
}

#[test]
fn extrapolation_modes() {
    let keys = || {
        vec![
            key(0.0, 0.0, Interpolation::Linear),
            key(2.0, 4.0, Interpolation::Linear),
        ]
    };
    let at = |mode, t| {
        Curve::new(keys(), mode, p())
            .and_then(|c| c.evaluate(t))
            .unwrap_or(f64::NAN)
    };
    assert!(near(at(Extrapolation::Constant, 5.0), 4.0));
    assert!(near(at(Extrapolation::Linear, 3.0), 6.0));
    assert!(near(at(Extrapolation::Linear, -1.0), -2.0));
    // Period 2: t=5 is local t=1 (value 2).
    assert!(near(at(Extrapolation::Cyclic, 5.0), 2.0));
    // t=5 is in the third cycle (local 1, value 2) after two 4-unit rises: 2 + 8.
    assert!(near(at(Extrapolation::CyclicOffset, 5.0), 10.0));
    assert!(near(at(Extrapolation::CyclicOffset, -1.0), -2.0));
}

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
