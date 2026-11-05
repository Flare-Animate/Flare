use flare_tween::{Curve, Easing, Extrapolation, HandleKind, Interpolation, Key, Params, ease};
use proptest::prelude::*;

const FAMILIES: [Interpolation; 11] = [
    Interpolation::Sine,
    Interpolation::Quad,
    Interpolation::Cubic,
    Interpolation::Quart,
    Interpolation::Quint,
    Interpolation::Expo,
    Interpolation::Circ,
    Interpolation::Back,
    Interpolation::Bounce,
    Interpolation::Elastic,
    Interpolation::Linear,
];

fn params() -> impl Strategy<Value = Params> {
    (0.0..=10.0f64, 1.0..=10.0f64, 0.01..=10.0f64).prop_map(|(b, a, p)| Params {
        back_overshoot: b,
        elastic_amplitude: a,
        elastic_period: p,
    })
}

proptest! {
    #[test]
    fn endpoints_are_exact_for_every_family_and_easing(p in params()) {
        for kind in FAMILIES {
            for easing in Easing::ALL {
                prop_assert_eq!(ease(kind, easing, 0.0, &p).ok(), Some(0.0));
                prop_assert_eq!(ease(kind, easing, 1.0, &p).ok(), Some(1.0));
            }
        }
    }

    #[test]
    fn no_nan_or_infinity_for_any_finite_input(t in -1e12..1e12f64, p in params()) {
        for kind in Interpolation::ALL {
            for easing in Easing::ALL {
                let v = ease(kind, easing, t, &p);
                prop_assert!(v.is_ok());
                prop_assert!(v.is_ok_and(f64::is_finite));
            }
        }
    }

    #[test]
    fn monotone_families_never_decrease(a in 0.0..=1.0f64, b in 0.0..=1.0f64) {
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        for kind in [Interpolation::Sine, Interpolation::Quad, Interpolation::Cubic,
                     Interpolation::Quart, Interpolation::Quint, Interpolation::Expo, Interpolation::Circ] {
            for easing in Easing::ALL {
                let x = ease(kind, easing, lo, &Params::default()).unwrap_or(f64::NAN);
                let y = ease(kind, easing, hi, &Params::default()).unwrap_or(f64::NAN);
                prop_assert!(x <= y + 1e-12, "{kind:?} {easing:?}");
            }
        }
    }

    #[test]
    fn auto_handles_keep_the_tangent_continuous(
        values in prop::collection::vec(-1000.0..1000.0f64, 3..12),
        kind in prop::sample::select(vec![HandleKind::Auto, HandleKind::AutoClamped]),
    ) {
        let keys: Vec<Key> = values
            .iter()
            .enumerate()
            .map(|(i, v)| Key { handle_kind: kind, ..Key::new(i as f64 * 3.0, *v) })
            .collect();
        let curve = Curve::new(keys, Extrapolation::Constant, Params::default())
            .map_err(|e| TestCaseError::fail(e.to_string()))?;
        for key in &curve.keys()[1..curve.keys().len() - 1] {
            // Both handles have the same slope, so the curve does not kink at the key.
            let l = key.left.dv / key.left.dt;
            let r = key.right.dv / key.right.dt;
            prop_assert!((l - r).abs() <= 1e-9 * (1.0 + l.abs()), "{l} vs {r}");
        }
    }

    #[test]
    fn curves_evaluate_finite_everywhere(
        mut times in prop::collection::vec(-1e4..1e4f64, 1..40),
        values in prop::collection::vec(-1e6..1e6f64, 40),
        t in -1e6..1e6f64,
        mode in prop::sample::select(Extrapolation::ALL.to_vec()),
        kind in prop::sample::select(Interpolation::ALL.to_vec()),
    ) {
        times.sort_by(f64::total_cmp);
        times.dedup();
        let keys: Vec<Key> = times
            .iter()
            .zip(values.iter())
            .map(|(time, value)| Key { interpolation: kind, ..Key::new(*time, *value) })
            .collect();
        if let Ok(curve) = Curve::new(keys, mode, Params::default()) {
            let v = curve.evaluate(t);
            prop_assert!(v.is_ok_and(f64::is_finite), "{v:?}");
        }
    }

    #[test]
    fn non_finite_time_is_always_refused(kind in prop::sample::select(Interpolation::ALL.to_vec())) {
        prop_assert!(ease(kind, Easing::Out, f64::NAN, &Params::default()).is_err());
        prop_assert!(ease(kind, Easing::Out, f64::INFINITY, &Params::default()).is_err());
    }
}
