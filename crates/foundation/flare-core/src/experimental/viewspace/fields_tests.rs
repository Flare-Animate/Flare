use super::*;
use crate::experimental::viewspace::head::{HeadParams, parametric_head};

type T = Result<(), ViewSpaceError>;

#[test]
fn linear_blend_and_normalisation() -> T {
    let (a, b) = ([0.0, 1.0], [1.0, 1.0]);
    let out = blend_fields(&[&a, &b], &[1.0, 3.0], FieldBlend::Linear)?;
    assert!((out[0] - 0.75).abs() < 1e-6 && (out[1] - 1.0).abs() < 1e-6);
    assert_eq!(
        blend_fields(&[&a, &b], &[0.0, 0.0], FieldBlend::Linear)?,
        vec![0.0, 1.0],
        "all zero gives the first"
    );
    Ok(())
}

#[test]
fn gamma_blend_favours_the_larger_value() -> T {
    let (a, b) = ([0.0], [1.0]);
    let lin = blend_fields(&[&a, &b], &[1.0, 1.0], FieldBlend::Linear)?[0];
    let g = blend_fields(&[&a, &b], &[1.0, 1.0], FieldBlend::Gamma(3.0))?[0];
    assert!(g > lin);
    assert!(blend_fields(&[&a, &b], &[1.0, 1.0], FieldBlend::Gamma(0.0)).is_err());
    assert!(blend_fields(&[&a, &b], &[1.0, 1.0], FieldBlend::Gamma(f32::NAN)).is_err());
    Ok(())
}

#[test]
fn blend_rejects_bad_input() {
    let a = [0.0, 1.0];
    assert!(blend_fields(&[], &[], FieldBlend::Linear).is_err());
    assert!(blend_fields(&[&a], &[1.0, 1.0], FieldBlend::Linear).is_err());
    assert!(blend_fields(&[&a, &[0.0]], &[1.0, 1.0], FieldBlend::Linear).is_err());
    assert!(blend_fields(&[&a], &[-1.0], FieldBlend::Linear).is_err());
    assert!(blend_fields(&[&a], &[f32::NAN], FieldBlend::Linear).is_err());
}

#[test]
fn field_defaults() {
    assert_eq!(field_default(THICKNESS), 1.0);
    assert_eq!(field_default(RIGIDITY), 1.0);
    assert_eq!(field_default(OCCLUSION), 0.0);
    assert_eq!(field_default("anything"), 0.0);
}

#[test]
fn painting_raises_lowers_sets_with_falloff() -> T {
    let m = parametric_head(&HeadParams::default())?;
    let mut f = m.default_depth.clone();
    let before = f.clone();
    let touched = paint_field(&m, &mut f, [0.0, 0.0], 0.5, 1.0, 0.1, DepthPaintMode::Raise)?;
    assert!(touched > 0 && touched < f.len());
    for (i, v) in m.vertices.iter().enumerate() {
        let inside = (v[0] * v[0] + v[1] * v[1]).sqrt() < 0.5;
        assert_eq!(f[i] != before[i], inside && before[i] < 1.0, "vertex {i}");
    }
    paint_field(&m, &mut f, [0.0, 0.0], 0.5, 1.0, 5.0, DepthPaintMode::Lower)?;
    assert!(f.iter().all(|v| (0.0..=1.0).contains(v)));
    paint_field(&m, &mut f, [0.0, 0.0], 0.5, 1.0, 0.25, DepthPaintMode::Set)?;
    let i = m
        .vertices
        .iter()
        .position(|v| v[0].abs() < 0.05 && v[1].abs() < 0.05 && v[2] > 0.0);
    assert!(i.is_some_and(|i| (f[i] - 0.25).abs() < 1e-6));
    Ok(())
}

#[test]
fn soft_brush_falls_off() -> T {
    let m = parametric_head(&HeadParams::default())?;
    let mut hard = vec![0.0; m.vertices.len()];
    let mut soft = vec![0.0; m.vertices.len()];
    paint_field(
        &m,
        &mut hard,
        [0.0, 0.0],
        1.0,
        1.0,
        0.5,
        DepthPaintMode::Raise,
    )?;
    paint_field(
        &m,
        &mut soft,
        [0.0, 0.0],
        1.0,
        0.0,
        0.5,
        DepthPaintMode::Raise,
    )?;
    assert!(soft.iter().sum::<f32>() < hard.iter().sum::<f32>());
    Ok(())
}

#[test]
fn paint_rejects_bad_input() -> T {
    let m = parametric_head(&HeadParams::default())?;
    let mut f = vec![0.0; m.vertices.len()];
    assert!(
        paint_field(
            &m,
            &mut f[1..],
            [0.0; 2],
            1.0,
            1.0,
            0.1,
            DepthPaintMode::Raise
        )
        .is_err()
    );
    assert!(paint_field(&m, &mut f, [0.0; 2], 0.0, 1.0, 0.1, DepthPaintMode::Raise).is_err());
    assert!(
        paint_field(
            &m,
            &mut f,
            [f32::NAN, 0.0],
            1.0,
            1.0,
            0.1,
            DepthPaintMode::Raise
        )
        .is_err()
    );
    let img = crate::experimental::viewspace::keyframe::Bitmap::filled(2, 2, [0; 4])?;
    assert!(matches!(
        auto_depth(&m, &img),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    Ok(())
}
