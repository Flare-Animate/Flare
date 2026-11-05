use super::*;
use crate::experimental::viewspace::head::{HeadParams, parametric_head};
use crate::experimental::viewspace::interp::RenderSettings;
use crate::experimental::viewspace::keyframe::{Bitmap, KeyframeSample, MeshAlignment};

type T = Result<(), ViewSpaceError>;

fn rig(with_sample: bool) -> Result<ViewSpaceInterpolator, ViewSpaceError> {
    let s = RenderSettings {
        width: 32,
        height: 32,
        scale: 12.0,
        ..RenderSettings::default()
    };
    let mut i = ViewSpaceInterpolator::new(parametric_head(&HeadParams::default())?, s)?;
    if with_sample {
        i.add_sample(KeyframeSample::new(
            Bitmap::filled(32, 32, [9; 4])?,
            ViewParams::default(),
            MeshAlignment {
                scale: 12.0,
                ..MeshAlignment::default()
            },
        ))?;
    }
    Ok(i)
}

#[test]
fn turntable_covers_a_full_turn() {
    let t = turntable(8, Some(10.0));
    assert_eq!(t.len(), 8);
    assert!((t[1].azimuth - t[0].azimuth - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
    assert!(turntable(0, None).is_empty());
    assert_eq!(turntable(MAX_SEQUENCE + 10, None).len(), MAX_SEQUENCE);
}

#[test]
fn sequence_renders_one_frame_per_view() -> T {
    let frames = render_sequence(
        &rig(true)?,
        &turntable(4, None),
        &ExpressionParams::default(),
        &ClothingState::default(),
    )?;
    assert_eq!(frames.len(), 4);
    assert!(frames.iter().all(|f| f.image.width == 32));
    Ok(())
}

#[test]
fn sequence_is_all_or_nothing_and_bounded() -> T {
    let views = turntable(4, None);
    assert_eq!(
        render_sequence(
            &rig(false)?,
            &views,
            &ExpressionParams::default(),
            &ClothingState::default()
        )
        .err(),
        Some(ViewSpaceError::NoSamples)
    );
    let many = vec![ViewParams::default(); MAX_SEQUENCE + 1];
    assert!(matches!(
        render_sequence(
            &rig(true)?,
            &many,
            &ExpressionParams::default(),
            &ClothingState::default()
        ),
        Err(ViewSpaceError::Limit { .. })
    ));
    Ok(())
}

#[test]
fn stubs_return_typed_errors() -> T {
    let i = rig(false)?;
    assert!(matches!(
        export_live2d_params(&i),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        export_grease_pencil(&i),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        export_runtime(&i),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    assert!(matches!(
        export_psd(&i),
        Err(ViewSpaceError::NotImplemented(_))
    ));
    Ok(())
}
