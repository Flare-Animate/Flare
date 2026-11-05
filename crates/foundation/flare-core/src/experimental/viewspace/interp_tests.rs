use super::*;
use crate::experimental::viewspace::head::{HeadParams, parametric_head};

type T = Result<(), ViewSpaceError>;

const SIZE: u32 = 128;

fn settings() -> RenderSettings {
    RenderSettings {
        width: SIZE,
        height: SIZE,
        scale: 40.0,
        ..RenderSettings::default()
    }
}

fn sample(az_deg: f32, rgb: [u8; 3]) -> Result<KeyframeSample, ViewSpaceError> {
    let img = Bitmap::filled(SIZE, SIZE, [rgb[0], rgb[1], rgb[2], 255])?;
    let align = MeshAlignment {
        scale: 40.0,
        ..MeshAlignment::default()
    };
    Ok(KeyframeSample::new(
        img,
        ViewParams::from_degrees(az_deg, None),
        align,
    ))
}

fn rig() -> Result<ViewSpaceInterpolator, ViewSpaceError> {
    let mut i = ViewSpaceInterpolator::new(parametric_head(&HeadParams::default())?, settings())?;
    i.add_sample(sample(0.0, [255, 0, 0])?)?;
    i.add_sample(sample(90.0, [0, 0, 255])?)?;
    Ok(i)
}

fn eval(i: &ViewSpaceInterpolator, az: f32) -> Result<Renderable2D, ViewSpaceError> {
    i.evaluate(
        &ViewParams::from_degrees(az, None),
        &ExpressionParams::default(),
        &ClothingState::default(),
    )
}

fn centre(r: &Renderable2D) -> [u8; 4] {
    let i = ((SIZE / 2 * SIZE + SIZE / 2) * 4) as usize;
    [
        r.image.data[i],
        r.image.data[i + 1],
        r.image.data[i + 2],
        r.image.data[i + 3],
    ]
}

fn sum_channel(r: &Renderable2D, ch: usize) -> u64 {
    r.image
        .data
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| u64::from(p[ch]))
        .sum()
}

#[test]
fn a_keyframe_view_reproduces_that_keyframe() -> T {
    let i = rig()?;
    assert_eq!(centre(&eval(&i, 0.0)?), [255, 0, 0, 255]);
    assert_eq!(centre(&eval(&i, 90.0)?), [0, 0, 255, 255]);
    Ok(())
}

#[test]
fn halfway_blends_both_and_the_blend_is_monotonic() -> T {
    let i = rig()?;
    let mid = centre(&eval(&i, 45.0)?);
    assert!(
        (100..160).contains(&mid[0]) && (100..160).contains(&mid[2]),
        "{mid:?}"
    );
    let blues: Vec<u8> = [0.0, 20.0, 40.0, 60.0, 80.0, 90.0]
        .iter()
        .map(|az| eval(&i, *az).map(|r| centre(&r)[2]))
        .collect::<Result<_, _>>()?;
    assert!(blues.windows(2).all(|w| w[0] <= w[1]), "{blues:?}");
    assert!(blues[0] == 0 && blues[5] == 255);
    Ok(())
}

#[test]
fn the_turn_wraps_around_the_back() -> T {
    let i = rig()?;
    // -30 degrees is nearer to the front sample than to the 90 degree one.
    assert!(centre(&eval(&i, -30.0)?)[0] > 200);
    Ok(())
}

#[test]
fn unseen_surface_degrades_to_grey_with_zero_confidence() -> T {
    let r = eval(&rig()?, 180.0)?;
    assert!(r.vertex_confidence.contains(&0.0));
    let c = centre(&r);
    assert_eq!(c[3], 255, "the head is still drawn");
    Ok(())
}

#[test]
fn no_samples_is_an_error() -> T {
    let empty = ViewSpaceInterpolator::new(parametric_head(&HeadParams::default())?, settings())?;
    assert_eq!(eval(&empty, 0.0).err(), Some(ViewSpaceError::NoSamples));
    let mut i = rig()?;
    i.settings.neighbours.max_distance = Some(0.01);
    assert_eq!(eval(&i, 45.0).err(), Some(ViewSpaceError::NoSamples));
    let bad = ViewParams {
        azimuth: f32::NAN,
        ..ViewParams::default()
    };
    assert!(
        rig()?
            .evaluate(
                &bad,
                &ExpressionParams::default(),
                &ClothingState::default()
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn confidence_peaks_on_keyframes_and_the_heatmap_is_optional() -> T {
    let mut i = rig()?;
    let on = eval(&i, 0.0)?;
    let between = eval(&i, 45.0)?;
    assert!(
        on.mean_confidence > between.mean_confidence,
        "{} vs {}",
        on.mean_confidence,
        between.mean_confidence
    );
    assert!(on.heatmap.is_none());
    i.settings.heatmap = true;
    let hm = eval(&i, 0.0)?.heatmap;
    assert!(hm.is_some_and(|h| h.data.as_chunks::<4>().0.iter().any(|p| p[3] == 255)));
    Ok(())
}

#[test]
fn pinning_a_region_takes_its_colour_from_one_keyframe() -> T {
    let mut i = rig()?;
    let free = sum_channel(&eval(&i, 45.0)?, 2);
    let red = i
        .samples()
        .next()
        .map(|s| s.id)
        .ok_or(ViewSpaceError::NoSamples)?;
    let regions: Vec<u16> = i.mesh().region_dict.keys().copied().collect();
    for r in &regions {
        i.pin_region(*r, red)?;
    }
    assert!(
        sum_channel(&eval(&i, 45.0)?, 2) < free,
        "pinned to red, less blue overall"
    );
    i.unpin_region(regions[0]);
    assert!(i.pin_region(999, red).is_err());
    assert!(i.pin_region(regions[0], KeyframeId::new()).is_err());
    Ok(())
}

#[test]
fn depth_override_moves_the_surface_towards_the_camera() -> T {
    let mut i = ViewSpaceInterpolator::new(parametric_head(&HeadParams::default())?, settings())?;
    let base = i.mesh().default_depth.clone();
    i.add_sample(sample(0.0, [9, 9, 9])?)?;
    let max = |r: &Renderable2D| r.depth.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let before = max(&eval(&i, 0.0)?);
    let mut s = sample(30.0, [9, 9, 9])?;
    s.depth_override = Some(base.iter().map(|d| d + 0.2).collect());
    i.add_sample(s)?;
    let after = max(&eval(&i, 30.0)?);
    assert!(after > before + 0.1, "{before} -> {after}");
    Ok(())
}

#[test]
fn thickness_scales_depth_and_occlusion_biases_it() -> T {
    let mut i = rig()?;
    let n = i.mesh().vertices.len();
    let max = |r: &Renderable2D| r.depth.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let plain = max(&eval(&i, 0.0)?);
    let mut s = sample(0.0, [255, 0, 0])?;
    s.id = KeyframeId::new();
    s.other_param_fields.insert(
        crate::experimental::viewspace::fields::THICKNESS.into(),
        vec![0.5; n],
    );
    s.other_param_fields.insert(
        crate::experimental::viewspace::fields::OCCLUSION.into(),
        vec![0.3; n],
    );
    s.clothing_state = Some("flat".into());
    i.add_sample(s)?;
    let flat = i.evaluate(
        &ViewParams::default(),
        &ExpressionParams::default(),
        &ClothingState::named("flat"),
    )?;
    assert!(
        max(&flat) < plain + 0.3 && max(&flat) > plain * 0.5 + 0.2,
        "thin but biased: {}",
        max(&flat)
    );
    Ok(())
}

#[test]
fn expressions_move_the_picture() -> T {
    let mut i = rig()?;
    let neutral = i.mesh().clone();
    let mut posed = neutral.clone();
    for v in &mut posed.vertices {
        v[0] += 0.5;
    }
    i.add_expression(ExpressionDelta::from_meshes("shift", &neutral, &posed)?)?;
    let cx = |r: &Renderable2D| {
        let (mut s, mut n) = (0.0f64, 0.0f64);
        for (k, p) in r.image.data.as_chunks::<4>().0.iter().enumerate() {
            if p[3] > 0 {
                s += (k % SIZE as usize) as f64;
                n += 1.0;
            }
        }
        s / n
    };
    let plain = eval(&i, 0.0)?;
    let shifted = i.evaluate(
        &ViewParams::default(),
        &ExpressionParams::default().with("shift", 1.0),
        &ClothingState::default(),
    )?;
    assert!(
        (cx(&shifted) - cx(&plain) - 20.0).abs() < 1.5,
        "0.5 units at 40 px per unit"
    );
    assert!(
        i.evaluate(
            &ViewParams::default(),
            &ExpressionParams::default().with("nope", 1.0),
            &ClothingState::default()
        )
        .is_err()
    );
    let mut bad = posed.clone();
    bad.vertices.pop();
    assert!(ExpressionDelta::from_meshes("x", &neutral, &bad).is_err());
    assert!(
        i.add_expression(ExpressionDelta {
            name: "short".into(),
            offsets: vec![]
        })
        .is_err()
    );
    Ok(())
}

#[test]
fn clothing_states_swap_independently() -> T {
    let mut i = rig()?;
    let mut coat = sample(0.0, [0, 255, 0])?;
    coat.clothing_state = Some("coat".into());
    i.add_sample(coat)?;
    let at = |c: &ClothingState| {
        i.evaluate(&ViewParams::default(), &ExpressionParams::default(), c)
            .map(|r| centre(&r))
    };
    assert_eq!(at(&ClothingState::default())?, [255, 0, 0, 255]);
    assert_eq!(at(&ClothingState::named("coat"))?, [0, 255, 0, 255]);
    assert_eq!(at(&ClothingState::named("unknown"))?, [255, 0, 0, 255]);
    Ok(())
}

#[test]
fn sample_management_is_bounded_and_checked() -> T {
    let mut i = rig()?;
    let s = sample(10.0, [1, 2, 3])?;
    let id = i.add_sample(s.clone())?;
    assert!(i.add_sample(s).is_err(), "duplicate id");
    assert!(i.remove_sample(id));
    assert!(!i.remove_sample(id));
    while i.samples().count() < MAX_SAMPLES {
        i.add_sample(sample(1.0, [0; 3])?)?;
    }
    assert!(matches!(
        i.add_sample(sample(1.0, [0; 3])?),
        Err(ViewSpaceError::Limit { .. })
    ));
    let mut bad = sample(0.0, [0; 3])?;
    bad.depth_override = Some(vec![0.0]);
    assert!(rig()?.add_sample(bad).is_err());
    let mut mesh = parametric_head(&HeadParams::default())?;
    mesh.faces.clear();
    assert!(ViewSpaceInterpolator::new(mesh, settings()).is_err());
    Ok(())
}
