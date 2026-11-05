use super::*;

#[test]
fn sharpening_keeps_a_distribution_and_orders_by_material() {
    let w = [0.7, 0.3];
    for m in [Material::Hair, Material::Skin, Material::Clothing] {
        let s = sharpen_weights(&w, material_blend_power(m));
        assert!((s.iter().sum::<f32>() - 1.0).abs() < 1e-5);
    }
    let hair = sharpen_weights(&w, material_blend_power(Material::Hair));
    let cloth = sharpen_weights(&w, material_blend_power(Material::Clothing));
    assert!(
        hair[0] < 0.7 && cloth[0] > 0.7,
        "hair blends softer than clothing"
    );
    assert_eq!(sharpen_weights(&w, 1.0), vec![0.7, 0.3]);
}

#[test]
fn sharpening_survives_bad_input() {
    assert_eq!(sharpen_weights(&[0.0, 0.0], 2.0), vec![0.0, 0.0]);
    assert_eq!(sharpen_weights(&[0.5, 0.5], f32::NAN), vec![0.5, 0.5]);
    assert_eq!(sharpen_weights(&[0.5, 0.5], -1.0), vec![0.5, 0.5]);
    assert!(sharpen_weights(&[], 2.0).is_empty());
}

#[test]
fn stubs_return_typed_errors() {
    use crate::experimental::viewspace::head::{HeadParams, parametric_head};
    let Ok(m) = parametric_head(&HeadParams::default()) else {
        return;
    };
    assert!(matches!(
        material_aware_weights(3),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        interpolate_strokes(),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        evaluate_gpu(),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        lod_settings(0),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        export_deltas_to_rig(&m),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        drive_mesh_from_bones(&m),
        Err(ViewSpaceError::NotImplemented(_))
    ));
}
