use super::*;
use crate::experimental::viewspace::head::{HeadParams, parametric_head};

type T = Result<(), ViewSpaceError>;

fn head() -> Result<BaseMesh, ViewSpaceError> {
    parametric_head(&HeadParams::default())
}

#[test]
fn validation_catches_each_kind_of_corruption() -> T {
    let ok = head()?;
    let mut m = ok.clone();
    m.faces[0][0] = 9_999_999;
    assert!(m.validate().is_err());
    let mut m = ok.clone();
    m.uvs.pop();
    assert!(m.validate().is_err());
    let mut m = ok.clone();
    m.vertices[3][1] = f32::INFINITY;
    assert!(m.validate().is_err());
    let mut m = ok.clone();
    m.region_ids[5] = 999;
    assert!(m.validate().is_err());
    let mut m = ok.clone();
    m.faces.clear();
    assert!(m.validate().is_err());
    let mut m = ok;
    m.bones = Some(BoneHierarchy {
        names: vec!["a".into(), "b".into()],
        parents: vec![Some(1), None],
        bind_positions: vec![[0.0; 3]; 2],
    });
    assert!(m.validate().is_err());
    Ok(())
}

#[test]
fn valid_bones_pass_and_serde_round_trips() -> T {
    let mut m = head()?;
    m.bones = Some(BoneHierarchy {
        names: vec!["neck".into(), "head".into()],
        parents: vec![None, Some(0)],
        bind_positions: vec![[0.0, -1.0, 0.0], [0.0, 0.0, 0.0]],
    });
    m.validate()?;
    let json = serde_json::to_string(&m).map_err(|e| ViewSpaceError::Invalid(e.to_string()))?;
    let back: BaseMesh =
        serde_json::from_str(&json).map_err(|e| ViewSpaceError::Invalid(e.to_string()))?;
    assert_eq!(back, m);
    Ok(())
}

#[test]
fn bounds_match_parameters() -> T {
    let (lo, hi) = head()?.bounds();
    assert!((hi[0] - 0.8).abs() < 0.02 && (lo[1] + 1.0).abs() < 1e-5);
    Ok(())
}

#[test]
fn stubs_return_typed_errors() -> T {
    let mut m = head()?;
    let mask = crate::experimental::viewspace::features::RegionMask::new(2, 2)?;
    assert!(matches!(
        m.snap_to_silhouette(&mask),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        m.apply_cage(&[]),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    let other = m.clone();
    assert!(matches!(
        m.attach(&other, "neck"),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    Ok(())
}
