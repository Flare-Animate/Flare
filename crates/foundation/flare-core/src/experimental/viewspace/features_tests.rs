use super::*;
use crate::experimental::viewspace::keyframe::{Bitmap, MeshAlignment, ViewParams};

type T = Result<(), ViewSpaceError>;

#[test]
fn hotkeys_map_in_order() {
    assert_eq!(FeaturePreset::from_hotkey('1'), Some(FeaturePreset::EyeL));
    assert_eq!(FeaturePreset::from_hotkey('0'), Some(FeaturePreset::ALL[9]));
    assert_eq!(FeaturePreset::from_hotkey('a'), None);
}

#[test]
fn custom_feature_names_are_checked() {
    assert!(FeatureId::custom("  ").is_err());
    assert!(FeatureId::custom(&"x".repeat(65)).is_err());
    assert_eq!(
        FeatureId::custom(" mole ").ok(),
        Some(FeatureId::Custom("mole".into()))
    );
}

#[test]
fn default_regions_exist_in_the_head() {
    assert_eq!(
        FeaturePreset::EyeR.default_region(),
        Some(crate::experimental::viewspace::mesh::region::R_EYE_SOCKET)
    );
    assert_eq!(FeaturePreset::Zipper.default_region(), None);
}

#[test]
fn mask_basics_and_centroid() -> T {
    let mut m = RegionMask::new(4, 4)?;
    assert_eq!(m.centroid(), None);
    m.set(1, 1, 255);
    m.set(2, 1, 255);
    m.set(-1, 0, 255);
    m.set(9, 9, 255);
    assert_eq!(m.area(), 2);
    assert_eq!(m.centroid(), Some([2.0, 1.5]));
    assert!(RegionMask::from_data(2, 2, vec![0; 3]).is_err());
    assert_eq!(m.get(-1, -1), 0);
    Ok(())
}

#[test]
fn materials_tag_selected_pixels() -> T {
    let mut mats = MaterialMap::new(4, 4)?;
    let mut m = RegionMask::new(4, 4)?;
    m.set(2, 2, 255);
    m.set(0, 0, 100);
    mats.assign(&m, Material::Hair)?;
    assert_eq!(mats.at(2, 2), Material::Hair);
    assert_eq!(
        mats.at(0, 0),
        Material::Unset,
        "soft pixels under 50% are not tagged"
    );
    assert_eq!(mats.at(99, 0), Material::Unset);
    assert!(
        mats.assign(&RegionMask::new(2, 2)?, Material::Skin)
            .is_err()
    );
    Ok(())
}

#[test]
fn assigning_a_feature_checks_size_and_count() -> T {
    let mut s = KeyframeSample::new(
        Bitmap::filled(4, 4, [0; 4])?,
        ViewParams::default(),
        MeshAlignment::default(),
    );
    assert!(
        assign_feature(
            &mut s,
            FeatureId::Preset(FeaturePreset::Nose),
            RegionMask::new(2, 2)?
        )
        .is_err()
    );
    assign_feature(
        &mut s,
        FeatureId::Preset(FeaturePreset::Nose),
        RegionMask::new(4, 4)?,
    )?;
    assert_eq!(s.feature_map.len(), 1);
    for i in 0..300 {
        let r = assign_feature(
            &mut s,
            FeatureId::Custom(format!("f{i}")),
            RegionMask::new(4, 4)?,
        );
        if i >= 255 {
            assert!(matches!(r, Err(ViewSpaceError::Limit { .. })));
        }
    }
    assert!(matches!(
        check_feature_consistency(&s, &s),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    Ok(())
}
