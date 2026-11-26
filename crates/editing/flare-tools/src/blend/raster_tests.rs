//! The Brush and the Eraser on a raster layer: a hard edge to start with, rubbing pixels out, and
//! the presets.
#![allow(clippy::expect_used)]

use flare_commands::Command;
use flare_core::Rgba8;
use flare_document::{Bitmap, Content, Stage};
use flare_input::InputSample;

use super::*;
use crate::drawing::Target;
use crate::ops::find_span;
use crate::test_support::{Fixture, at, fixture};

const BLUE: Rgba8 = Rgba8::opaque(0, 0, 255);

fn stage() -> Fixture {
    let mut f = fixture();
    let stage = Stage::new(64, 48, Rgba8::opaque(255, 255, 255)).expect("stage");
    f.session
        .apply([Command::SetStage(stage)])
        .expect("set stage");
    f
}

fn line(points: &[(f64, f64)]) -> Vec<InputSample> {
    points
        .iter()
        .map(|(x, y)| InputSample::mouse(*x, *y).expect("finite"))
        .collect()
}

fn paint_line(f: &mut Fixture, settings: BlendSettings, points: &[(f64, f64)]) {
    let paint = RasterPaint {
        color: BLUE,
        layer: Target::Layer(f.layer),
        new_layer: "Layer 1".into(),
        bitmap_name: "Painting".into(),
    };
    let edit = lay_paint(
        f.session.document(),
        f.scene,
        0,
        settings,
        paint,
        &line(points),
    )
    .expect("paint");
    f.session.apply(edit.commands).expect("apply");
}

fn painted(f: &Fixture) -> Bitmap {
    let span = find_span(f.session.document(), f.scene, at(f, 0)).expect("a keyframe");
    match span.element(0) {
        Some(Content::Bitmap(placed)) => f
            .session
            .document()
            .bitmap(placed.bitmap())
            .expect("in the library")
            .clone(),
        other => panic!("not a bitmap: {other:?}"),
    }
}

/// The alpha of the painting's pixel `(x, y)`, in its own pixels.
fn alpha(bitmap: &Bitmap, x: u32, y: u32) -> u8 {
    bitmap.pixels()[((y * bitmap.width() + x) * 4 + 3) as usize]
}

#[test]
fn the_brush_starts_hard_so_its_edge_is_sharp() {
    let mut f = stage();
    let brush = BlendSettings::for_kind(BlendKind::RasterBrush);
    assert_eq!(brush.hardness, 1.0);
    assert_eq!(brush.mix, 0.0, "the paint lays as it is");
    let size = BlendSettings {
        size: 10.0,
        ..brush
    };
    paint_line(&mut f, size, &[(8.0, 24.0), (56.0, 24.0)]);
    let painting = painted(&f);
    // Ten stage units wide is twenty pixels: a radius of ten about row 48.
    let d = PAINT_DENSITY;
    let (x, middle) = (32 * d, 24 * d);
    assert_eq!(
        alpha(&painting, x, middle + 8),
        255,
        "solid to near the edge"
    );
    assert_eq!(
        alpha(&painting, x, middle + 11),
        0,
        "and nothing just past it"
    );
}

#[test]
fn the_eraser_rubs_pixels_out_of_the_painting_and_leaves_the_rest() {
    let mut f = stage();
    paint_line(
        &mut f,
        BlendSettings {
            size: 20.0,
            ..BlendSettings::for_kind(BlendKind::RasterBrush)
        },
        &[(4.0, 24.0), (60.0, 24.0)],
    );
    let before = painted(&f);
    let eraser = BlendSettings {
        size: 8.0,
        ..BlendSettings::for_kind(BlendKind::Erase)
    };
    let edit = rub_out(
        f.session.document(),
        f.scene,
        0,
        eraser,
        Some(f.layer),
        &line(&[(32.0, 10.0), (32.0, 38.0)]),
    )
    .expect("erase");
    f.session.apply(edit.commands).expect("apply");
    let after = painted(&f);
    assert_ne!(before.id(), after.id(), "a new picture, as a stroke makes");
    assert_eq!(f.session.document().bitmaps().len(), 1, "the old one went");
    let d = PAINT_DENSITY;
    assert_eq!(alpha(&after, 32 * d, 24 * d), 0, "rubbed out");
    assert_eq!(alpha(&after, 10 * d, 24 * d), 255, "the rest is kept");
    assert!(f.session.undo(), "one step");
    assert_eq!(alpha(&painted(&f), 32 * d, 24 * d), 255);
}

#[test]
fn the_eraser_needs_a_painting_under_it() {
    let f = stage();
    let result = rub_out(
        f.session.document(),
        f.scene,
        0,
        BlendSettings::for_kind(BlendKind::Erase),
        Some(f.layer),
        &line(&[(32.0, 24.0)]),
    );
    assert_eq!(result.err(), Some(BlendError::NothingToErase));
}

#[test]
fn every_preset_is_valid_named_and_found_again_from_its_settings() {
    assert_eq!(
        RasterPreset::Round.settings(),
        BlendSettings::for_kind(BlendKind::RasterBrush),
        "a new brush is the round one"
    );
    for preset in RasterPreset::ALL {
        preset.settings().validate().expect("valid");
        assert_eq!(RasterPreset::parse(preset.name()), Some(preset));
        assert_eq!(RasterPreset::parse(preset.label()), Some(preset));
        assert_eq!(
            RasterPreset::of(&preset.settings()),
            Some(preset),
            "{preset:?}"
        );
    }
    let airbrush = RasterPreset::Airbrush.settings();
    assert!(
        airbrush.hardness < 0.1 && airbrush.strength < 0.5,
        "soft and faint"
    );
    assert!(RasterPreset::Blender.settings().mix > 0.9, "only blends");
    assert_eq!(RasterPreset::parse("crayon"), None);
    let tweaked = BlendSettings {
        size: 21.0,
        ..RasterPreset::Round.settings()
    };
    assert_eq!(
        RasterPreset::of(&tweaked),
        None,
        "a changed preset is no longer it"
    );
}

#[test]
fn a_swatch_shows_each_presets_mark_in_white_and_a_bad_size_is_refused() {
    let alpha = |rgba: &[u8]| rgba.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count();
    let swatch = |p: RasterPreset| raster_swatch(p.settings(), 160, 40, 6).expect("swatch");
    let round = swatch(RasterPreset::Round);
    assert_eq!(round.len(), 160 * 40 * 4);
    // White, so the panel tints it as it tints the vector Brush's swatches.
    assert!(
        round
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| p[3] > 0)
            .all(|p| p[..3] == [255, 255, 255]),
        "the mark is white"
    );
    // Laid inside the margin: nothing in the first and last columns but the brush's own spread.
    let column =
        |rgba: &[u8], x: usize| (0..40).filter(|y| rgba[(y * 160 + x) * 4 + 3] > 0).count();
    assert_eq!(column(&round, 0), 0, "the margin is clear");
    assert!(column(&round, 80) > 0, "the middle is painted");
    let pencil = swatch(RasterPreset::Pencil);
    let airbrush = swatch(RasterPreset::Airbrush);
    assert!(alpha(&pencil) > 0, "the pencil draws");
    assert!(
        alpha(&pencil) < alpha(&round),
        "a pencil is thinner than a round brush"
    );
    assert!(alpha(&airbrush) > alpha(&round), "an airbrush is wider");
    let faint = airbrush
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3])
        .max()
        .unwrap_or(0);
    assert!(faint < 128, "and faint: {faint}");
    // A brush that mixes is drawn over a grey band, which it smears into greys between.
    let wet = swatch(RasterPreset::WetPaint);
    assert!(alpha(&wet) >= (160 - 12) * 10, "the band shows");
    let between = wet
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| p[3] == 255 && p[0] > 110 && p[0] < 250);
    assert!(between, "the paint and the band mix");
    for (w, h, pad) in [(4, 40, 0), (160, 40, 20), (2000, 40, 0)] {
        assert_eq!(
            raster_swatch(RasterPreset::Round.settings(), w, h, pad),
            Err(BlendError::Size)
        );
    }
}
