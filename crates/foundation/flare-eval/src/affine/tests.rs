#![allow(clippy::expect_used)]
use super::*;
use proptest::prelude::*;

fn close(a: DAffine2, b: DAffine2, eps: f64) -> bool {
    a.to_cols_array()
        .iter()
        .zip(b.to_cols_array())
        .all(|(x, y)| (x - y).abs() <= eps * (1.0 + y.abs()))
}

fn matrix() -> impl Strategy<Value = DAffine2> {
    prop::array::uniform6(-100.0f64..100.0).prop_map(|v| DAffine2::from_cols_array(&v))
}

proptest! {
    #[test]
    fn decompose_then_recompose_is_the_matrix(m in matrix()) {
        let parts = decompose(m).expect("finite");
        prop_assert!(close(parts.matrix(), m, 1e-9));
    }

    #[test]
    fn endpoints_are_exact(a in matrix(), b in matrix()) {
        prop_assert_eq!(interpolate(a, b, 0.0, None), Some(a));
        prop_assert_eq!(interpolate(a, b, 1.0, None), Some(b));
        prop_assert_eq!(interpolate(a, a, 0.37, None), Some(a));
    }

    #[test]
    fn midpoint_is_finite_and_between_the_translations(a in matrix(), b in matrix()) {
        let mid = interpolate(a, b, 0.5, None).expect("finite");
        prop_assert!(mid.is_finite());
        let expected = (a.translation + b.translation) / 2.0;
        prop_assert!((mid.translation - expected).length() < 1e-9);
    }
}

#[test]
fn a_reflection_keeps_its_handedness_across_the_span() {
    let flip = DAffine2::from_cols_array(&[1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
    let turned = DAffine2::from_cols_array(&[0.0, 1.0, 1.0, 0.0, 10.0, 0.0]); // also a reflection
    for step in 0..=10 {
        let m = interpolate(flip, turned, f64::from(step) / 10.0, None).expect("ok");
        assert!(m.matrix2.determinant() <= 1e-12, "step {step}");
    }
}

#[test]
fn changing_handedness_passes_through_a_collapsed_axis() {
    let plain = DAffine2::IDENTITY;
    let flip = DAffine2::from_cols_array(&[1.0, 0.0, 0.0, -1.0, 0.0, 0.0]);
    let mid = interpolate(plain, flip, 0.5, None).expect("ok");
    assert!(mid.matrix2.determinant().abs() < 1e-12);
}

#[test]
fn rotation_shear_and_reflection_midpoint_is_sane() {
    let a = DAffine2::from_cols_array(&[1.0, 0.0, 0.6, 1.0, 0.0, 0.0]);
    let rot = DAffine2::from_angle(1.0);
    let b = DAffine2::from_cols(
        rot.matrix2.x_axis * 2.0,
        -rot.matrix2.y_axis * 2.0,
        DVec2::ONE,
    );
    let mid = interpolate(a, b, 0.5, None).expect("ok");
    let det = mid.matrix2.determinant();
    assert!(det.is_finite() && det.abs() < 4.0);
}

#[test]
fn degenerate_matrices_decompose() {
    for m in [
        DAffine2::from_cols_array(&[0.0; 6]),
        DAffine2::from_cols_array(&[0.0, 0.0, 1.0, 2.0, 3.0, 4.0]),
        DAffine2::from_cols_array(&[1.0, 2.0, 2.0, 4.0, 0.0, 0.0]),
    ] {
        let p = decompose(m).expect("finite");
        assert!(close(p.matrix(), m, 1e-9), "{m:?}");
    }
    assert!(decompose(DAffine2::from_translation(DVec2::new(f64::NAN, 0.0))).is_none());
}
