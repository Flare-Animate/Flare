use super::*;
use crate::experimental::viewspace::head::{HeadParams, parametric_head};
use crate::experimental::viewspace::keyframe::{Bitmap, MeshAlignment, ViewParams, register};

type T = Result<(), ViewSpaceError>;

fn reg(state: Option<&str>, expr: Option<&str>) -> Result<Registered, ViewSpaceError> {
    let m = parametric_head(&HeadParams::default())?;
    let mut s = KeyframeSample::new(
        Bitmap::filled(8, 8, [1; 4])?,
        ViewParams::default(),
        MeshAlignment::default(),
    );
    s.clothing_state = state.map(String::from);
    s.expression_tag = expr.map(String::from);
    register(&m, s)
}

#[test]
fn own_state_wins_and_base_is_the_fallback() -> T {
    let all = [
        reg(None, None)?,
        reg(Some("coat"), None)?,
        reg(Some("cape"), None)?,
    ];
    assert_eq!(usable_samples(&all, &ClothingState::named("coat")), vec![1]);
    assert_eq!(
        usable_samples(&all, &ClothingState::named("armour")),
        vec![0],
        "unknown state falls back to shared"
    );
    assert_eq!(usable_samples(&all, &ClothingState::default()), vec![0]);
    Ok(())
}

#[test]
fn expression_samples_are_not_view_samples() -> T {
    let all = [reg(None, Some("smile"))?, reg(None, None)?];
    assert_eq!(usable_samples(&all, &ClothingState::default()), vec![1]);
    assert!(usable_samples(&[], &ClothingState::default()).is_empty());
    Ok(())
}

#[test]
fn stubs_return_typed_errors() -> T {
    let r = reg(None, None)?;
    assert!(matches!(
        duplicate_body_mapping(&r.sample, "coat"),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    let m = parametric_head(&HeadParams::default())?;
    assert!(matches!(
        suggest_occlusion(&m, &r.sample),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    Ok(())
}
