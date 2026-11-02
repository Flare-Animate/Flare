//! Properties of pointer samples and the pressure curve, over many generated inputs.
#![allow(clippy::expect_used)]

use flare_input::{InputSample, PointerKind, PressureCurve};
use proptest::prelude::*;

fn kind() -> impl Strategy<Value = PointerKind> {
    prop_oneof![
        Just(PointerKind::Mouse),
        Just(PointerKind::Pen),
        Just(PointerKind::Touch)
    ]
}

proptest! {
    #[test]
    fn a_sample_is_made_exactly_when_the_position_is_finite_and_its_pressure_is_always_usable(
        x in any::<f64>(), y in any::<f64>(), pressure in any::<f32>(), kind in kind(),
    ) {
        let sample = InputSample::new(x, y, pressure, kind);
        prop_assert_eq!(sample.is_some(), x.is_finite() && y.is_finite());
        if let Some(sample) = sample {
            prop_assert!((0.0..=1.0).contains(&sample.pressure));
            prop_assert_eq!((sample.x, sample.y, sample.kind), (x, y, kind));
        }
    }

    #[test]
    fn harder_pressure_never_makes_a_thinner_line_and_widths_stay_between_the_minimum_and_one(
        min in any::<f32>(), gamma in any::<f32>(), a in any::<f32>(), b in any::<f32>(),
    ) {
        let curve = PressureCurve::new(min, gamma);
        let (lo, hi) = if a.is_nan() || b.is_nan() { (0.0, 1.0) } else if a <= b { (a, b) } else { (b, a) };
        let (fl, fh) = (curve.width_factor(lo), curve.width_factor(hi));
        prop_assert!(fl <= fh + 1e-6, "{fl} then {fh}");
        for f in [fl, fh] {
            prop_assert!(f.is_finite() && (0.0..=1.0).contains(&f), "{f}");
        }
    }
}
