use super::*;
use crate::experimental::viewspace::clothing::ClothingState;
use crate::experimental::viewspace::expression::ExpressionParams;
use crate::experimental::viewspace::head::{HeadParams, parametric_head};
use crate::experimental::viewspace::interp::RenderSettings;

type T = Result<(), ViewSpaceError>;

fn rig() -> Result<ViewSpaceInterpolator, ViewSpaceError> {
    let s = RenderSettings {
        width: 96,
        height: 96,
        scale: 30.0,
        ..RenderSettings::default()
    };
    let mut i = ViewSpaceInterpolator::new(parametric_head(&HeadParams::default())?, s)?;
    let align = MeshAlignment {
        scale: 30.0,
        ..MeshAlignment::default()
    };
    i.add_sample(KeyframeSample::new(
        Bitmap::filled(96, 96, [255, 0, 0, 255])?,
        ViewParams::from_degrees(0.0, None),
        align,
    ))?;
    i.add_sample(KeyframeSample::new(
        Bitmap::filled(96, 96, [0, 0, 255, 255])?,
        ViewParams::from_degrees(90.0, None),
        align,
    ))?;
    Ok(i)
}

fn eval(i: &ViewSpaceInterpolator, az: f32) -> Result<Vec<u8>, ViewSpaceError> {
    Ok(i.evaluate(
        &ViewParams::from_degrees(az, None),
        &ExpressionParams::default(),
        &ClothingState::default(),
    )?
    .image
    .data)
}

#[test]
fn generate_correct_regenerate() -> T {
    let mut i = rig()?;
    let view = ViewParams::from_degrees(45.0, None);
    let mut frame = i
        .evaluate(
            &view,
            &ExpressionParams::default(),
            &ClothingState::default(),
        )?
        .image;
    let centre = (48 * 96 + 48) * 4;
    let wrong = frame.data[centre..centre + 3].to_vec();
    // The artist repaints the whole frame green.
    for p in frame.data.as_chunks_mut::<4>().0.iter_mut() {
        p.copy_from_slice(&[0, 255, 0, 255]);
    }
    let sample = corrective_keyframe(&i, view, frame)?;
    assert!(sample.is_corrective);
    let id = commit_corrective(&mut i, sample)?;
    let fixed = eval(&i, 45.0)?;
    assert_eq!(
        fixed[centre..centre + 3],
        [0, 255, 0],
        "the correction replaces {wrong:?} at its own view"
    );
    // Far from the correction the other keyframes still rule.
    assert_eq!(eval(&i, 0.0)?[centre..centre + 3], [255, 0, 0]);
    assert!(i.remove_sample(id));
    assert_eq!(eval(&i, 45.0)?[centre..centre + 3], wrong[..]);
    Ok(())
}

#[test]
fn rejects_wrong_size_and_non_corrective() -> T {
    let mut i = rig()?;
    assert!(
        corrective_keyframe(&i, ViewParams::default(), Bitmap::filled(10, 10, [0; 4])?).is_err()
    );
    let plain = KeyframeSample::new(
        Bitmap::filled(96, 96, [0; 4])?,
        ViewParams::default(),
        MeshAlignment::default(),
    );
    assert!(commit_corrective(&mut i, plain).is_err());
    Ok(())
}

#[test]
fn push_back_is_a_typed_stub() -> T {
    let mut i = rig()?;
    assert!(matches!(
        push_back_to_shared(&mut i, KeyframeId::new()),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    Ok(())
}
