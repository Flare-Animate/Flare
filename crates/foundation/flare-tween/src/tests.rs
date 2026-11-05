use crate::bezier::cubic_at;
use crate::{Curve, Easing, Extrapolation, Interpolation, Key, MAX_KEYS, Params, TweenError, ease};

pub(super) const EPS: f64 = 1e-9;

pub(super) fn p() -> Params {
    Params::default()
}

pub(super) fn key(time: f64, value: f64, kind: Interpolation) -> Key {
    Key {
        interpolation: kind,
        ..Key::new(time, value)
    }
}

pub(super) fn near(a: f64, b: f64) -> bool {
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
