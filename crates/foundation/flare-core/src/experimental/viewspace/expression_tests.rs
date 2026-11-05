use super::*;
use crate::experimental::viewspace::head::{HeadParams, parametric_head};
use crate::experimental::viewspace::keyframe::{Bitmap, KeyframeSample, MeshAlignment, ViewParams};

type T = Result<(), ViewSpaceError>;

fn pair() -> Result<(BaseMesh, BaseMesh), ViewSpaceError> {
    let a = parametric_head(&HeadParams::default())?;
    let b = parametric_head(&HeadParams {
        jaw_width: 1.0,
        ..HeadParams::default()
    })?;
    Ok((a, b))
}

#[test]
fn delta_is_posed_minus_neutral() -> T {
    let (a, b) = pair()?;
    let d = ExpressionDelta::from_meshes(" wide jaw ", &a, &b)?;
    assert_eq!(d.name, "wide jaw");
    let mut p = a.vertices.clone();
    let deltas = BTreeMap::from([(d.name.clone(), d)]);
    apply_expression(
        &mut p,
        &deltas,
        &ExpressionParams::default().with("wide jaw", 1.0),
    )?;
    for (x, y) in p.iter().zip(&b.vertices) {
        for k in 0..3 {
            assert!((x[k] - y[k]).abs() < 1e-5);
        }
    }
    Ok(())
}

#[test]
fn half_weight_is_halfway_and_zero_is_neutral() -> T {
    let (a, b) = pair()?;
    let d = ExpressionDelta::from_meshes("e", &a, &b)?;
    let deltas = BTreeMap::from([("e".to_string(), d)]);
    let i = a
        .vertices
        .iter()
        .position(|v| v[1] < -0.5 && v[0] > 0.1)
        .unwrap_or(0);
    let mut half = a.vertices.clone();
    apply_expression(
        &mut half,
        &deltas,
        &ExpressionParams::default().with("e", 0.5),
    )?;
    assert!((half[i][0] - (a.vertices[i][0] + b.vertices[i][0]) / 2.0).abs() < 1e-5);
    let mut zero = a.vertices.clone();
    apply_expression(
        &mut zero,
        &deltas,
        &ExpressionParams::default().with("e", 0.0),
    )?;
    assert_eq!(zero, a.vertices);
    Ok(())
}

#[test]
fn errors_instead_of_silent_neutral() -> T {
    let (a, b) = pair()?;
    let d = ExpressionDelta::from_meshes("e", &a, &b)?;
    let deltas = BTreeMap::from([("e".to_string(), d)]);
    let mut p = a.vertices.clone();
    assert!(
        apply_expression(
            &mut p,
            &deltas,
            &ExpressionParams::default().with("typo", 1.0)
        )
        .is_err()
    );
    assert!(
        apply_expression(&mut p, &deltas, &ExpressionParams::default().with("e", 3.0)).is_err()
    );
    assert!(
        apply_expression(
            &mut p,
            &deltas,
            &ExpressionParams::default().with("e", f32::NAN)
        )
        .is_err()
    );
    assert!(
        apply_expression(
            &mut p[1..].to_vec(),
            &deltas,
            &ExpressionParams::default().with("e", 1.0)
        )
        .is_err()
    );
    assert!(ExpressionDelta::from_meshes(" ", &a, &b).is_err());
    let other = parametric_head(&HeadParams {
        rings: 6,
        ..HeadParams::default()
    })?;
    assert!(ExpressionDelta::from_meshes("e", &a, &other).is_err());
    Ok(())
}

#[test]
fn stubs_return_typed_errors() -> T {
    let s = KeyframeSample::new(
        Bitmap::filled(2, 2, [0; 4])?,
        ViewParams::default(),
        MeshAlignment::default(),
    );
    assert!(matches!(
        deltas_from_keyframes(&s, &s),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        blend_view_deltas("e", &[]),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    Ok(())
}
