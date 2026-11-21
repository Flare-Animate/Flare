#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{BitmapId, InstanceId, LayerId, Rgba8, Transform2D};
use flare_document::{Bitmap, BitmapInstance, Content, FrameSpan, Layer, Stage};
use flare_input::InputSample;

use super::*;
use crate::common::find_element;
use crate::ops::find_span;
use crate::test_support::{Fixture, fixture};

const BLUE: Rgba8 = Rgba8::opaque(0, 0, 255);
const RED: [u8; 4] = [255, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];

/// The fixture on a 64 by 48 stage.
fn stage() -> Fixture {
    let mut f = fixture();
    let stage = Stage::new(64, 48, Rgba8::opaque(255, 255, 255)).expect("stage");
    f.session
        .apply([Command::SetStage(stage)])
        .expect("set stage");
    f
}

/// A `w` by `h` picture of one color on the fixture's layer at frames 0 to 4, at the origin.
fn picture(f: &mut Fixture, w: u32, h: u32, color: [u8; 4]) -> BitmapId {
    let rgba = (0..w * h).flat_map(|_| color).collect();
    let bitmap = Bitmap::new(BitmapId::new(), "picture", w, h, rgba).expect("bitmap");
    let id = bitmap.id();
    let placed = BitmapInstance::new(InstanceId::new(), id, Transform2D::IDENTITY);
    let five = NonZeroU32::new(5).expect("five");
    f.session
        .apply([
            Command::AddBitmap(bitmap),
            Command::InsertSpan {
                scene: f.scene,
                layer: f.layer,
                span: FrameSpan::new(0, five, Content::Bitmap(placed)),
            },
        ])
        .expect("place");
    id
}

fn paint(f: &Fixture, color: Rgba8) -> RasterPaint {
    RasterPaint {
        color,
        layer: Target::Layer(f.layer),
        new_layer: "Layer 1".into(),
        bitmap_name: "Painting".into(),
    }
}

fn brush(mix: f32) -> BlendSettings {
    BlendSettings {
        size: 10.0,
        hardness: 1.0,
        mix,
        ..BlendSettings::for_kind(BlendKind::RasterBrush)
    }
}

fn line(points: &[(f64, f64)]) -> Vec<InputSample> {
    points
        .iter()
        .map(|(x, y)| InputSample::mouse(*x, *y).expect("finite"))
        .collect()
}

/// Paints `points` and applies the edit.
fn stroke(f: &mut Fixture, settings: BlendSettings, color: Rgba8, points: &[(f64, f64)]) {
    let edit = lay_paint(
        f.session.document(),
        f.scene,
        0,
        settings,
        paint(f, color),
        &line(points),
    )
    .expect("paint");
    f.session.apply(edit.commands).expect("apply");
}

/// The bitmap the topmost element of the fixture's layer at frame 0 shows.
fn painted(f: &Fixture) -> Bitmap {
    let span = find_span(f.session.document(), f.scene, crate::test_support::at(f, 0))
        .expect("a keyframe at 0");
    let top = span.element_count() - 1;
    match span.element(top) {
        Some(Content::Bitmap(placed)) => f
            .session
            .document()
            .bitmap(placed.bitmap())
            .expect("in the library")
            .clone(),
        other => panic!("not a bitmap: {other:?}"),
    }
}

fn pixel(bitmap: &Bitmap, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * bitmap.width() + x) * 4) as usize;
    let p = &bitmap.pixels()[i..i + 4];
    [p[0], p[1], p[2], p[3]]
}

#[test]
fn a_stroke_on_an_empty_layer_paints_a_new_bitmap_the_size_of_the_stage() {
    let mut f = stage();
    // Clear pixels have no color to give, so even a full mix lays the paint as it is.
    stroke(&mut f, brush(1.0), BLUE, &[(8.0, 24.0), (56.0, 24.0)]);
    let after = painted(&f);
    assert_eq!((after.width(), after.height()), (64, 48));
    assert_eq!(after.name(), "Painting");
    assert_eq!(pixel(&after, 32, 24), [0, 0, 255, 255]);
    assert_eq!(pixel(&after, 32, 2), [0, 0, 0, 0], "above the stroke");
    assert_eq!(f.session.document().bitmaps().len(), 1);
    // The whole stroke is one step: undone, the bitmap and the keyframe are gone.
    assert!(f.session.undo());
    assert!(f.session.document().bitmaps().is_empty());
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    assert!(layer.span_at(0).is_none());
}

/// A blue stroke across a red picture painted with `mix`: the color at its middle.
fn across_red(mix: f32) -> Result<[u8; 4], BlendError> {
    let mut f = stage();
    picture(&mut f, 64, 48, RED);
    let edit = lay_paint(
        f.session.document(),
        f.scene,
        0,
        brush(mix),
        paint(&f, BLUE),
        &line(&[(4.0, 24.0), (60.0, 24.0)]),
    )?;
    f.session.apply(edit.commands).expect("apply");
    Ok(pixel(&painted(&f), 32, 24))
}

#[test]
fn the_mix_blends_the_paint_with_the_color_under_it() {
    assert_eq!(
        across_red(0.0),
        Ok([0, 0, 255, 255]),
        "no mix: the paint covers"
    );
    let half = across_red(0.5).expect("half");
    assert!(
        (100..=155).contains(&half[0]) && (100..=155).contains(&half[2]),
        "half and half: {half:?}"
    );
    let more = across_red(0.8).expect("more");
    assert!(more[0] > half[0] + 40, "more mix, more red: {more:?}");
    // All mix only moves what is there, and red moved over red is red.
    assert_eq!(across_red(1.0), Err(BlendError::NothingChanged));
}

#[test]
fn a_second_stroke_paints_into_the_same_picture_and_mixes_with_the_first() {
    let mut f = stage();
    stroke(
        &mut f,
        brush(0.0),
        Rgba8::opaque(255, 0, 0),
        &[(4.0, 24.0), (60.0, 24.0)],
    );
    let first = painted(&f).id();
    stroke(&mut f, brush(0.5), BLUE, &[(32.0, 2.0), (32.0, 46.0)]);
    let span = find_span(
        f.session.document(),
        f.scene,
        crate::test_support::at(&f, 0),
    )
    .expect("span");
    assert_eq!(span.element_count(), 1, "one picture, painted twice");
    let library = f.session.document().bitmaps();
    assert_eq!(library.len(), 1, "the first picture left the library");
    assert_ne!(library[0].id(), first);
    let after = painted(&f);
    let crossing = pixel(&after, 32, 24);
    // The brush picks the red up as it reaches it, so a thin line tints it less than a wide
    // field would, and it carries some on past the line.
    assert!(
        crossing[0] >= 20 && crossing[2] > 150,
        "where they cross the colors mix: {crossing:?}"
    );
    let past = pixel(&after, 32, 34);
    assert!(past[0] > 0, "red carried on past the line: {past:?}");
    assert_eq!(
        pixel(&after, 32, 6),
        [0, 0, 255, 255],
        "over clear it is blue"
    );
    assert_eq!(
        pixel(&after, 8, 24),
        RED,
        "the red away from the crossing is kept"
    );
}

#[test]
fn opacity_caps_how_far_the_paint_covers() {
    let mut f = stage();
    picture(&mut f, 64, 48, WHITE);
    let half = BlendSettings {
        strength: 0.5,
        ..brush(0.0)
    };
    stroke(
        &mut f,
        half,
        BLUE,
        &[(4.0, 24.0), (32.0, 24.0), (4.0, 24.0)],
    );
    let p = pixel(&painted(&f), 16, 24);
    assert!(
        (120..=136).contains(&p[0]) && p[2] == 255,
        "half way from white to blue, however often it is gone over: {p:?}"
    );
}

#[test]
fn a_click_paints_a_dot() {
    let mut f = stage();
    stroke(&mut f, brush(0.5), BLUE, &[(20.0, 20.0)]);
    let after = painted(&f);
    assert_eq!(pixel(&after, 20, 20), [0, 0, 255, 255]);
    assert_eq!(pixel(&after, 40, 20), [0, 0, 0, 0]);
}

#[test]
fn it_paints_its_own_layer_and_leaves_a_picture_on_another_alone() {
    let mut f = stage();
    let other = picture(&mut f, 64, 48, RED);
    let top = LayerId::new();
    f.session
        .apply([Command::AddLayer {
            scene: f.scene,
            layer: Layer::new(top, "Top", vec![]).expect("layer"),
        }])
        .expect("layer");
    let target = RasterPaint {
        layer: Target::Layer(top),
        ..paint(&f, BLUE)
    };
    let edit = lay_paint(
        f.session.document(),
        f.scene,
        0,
        brush(1.0),
        target,
        &line(&[(4.0, 24.0), (60.0, 24.0)]),
    )
    .expect("paint");
    assert_eq!(edit.at.layer, top);
    f.session.apply(edit.commands).expect("apply");
    let doc = f.session.document();
    assert!(doc.bitmap(other).is_some(), "the red picture is untouched");
    let Some(Content::Bitmap(placed)) = find_element(doc, f.scene, edit.at) else {
        panic!("the painting is on the top layer");
    };
    let painting = doc.bitmap(placed.bitmap()).expect("painting");
    // Only the bitmap's own pixels mix, and the new one held none.
    assert_eq!(pixel(painting, 32, 24), [0, 0, 255, 255]);
}

#[test]
fn a_stroke_applied_as_it_goes_places_its_bitmap_once_and_paints_on() {
    let mut f = stage();
    let start = Pos2::new(4.0, 24.0);
    let mut live = BlendStroke::paint(
        f.session.document(),
        f.scene,
        0,
        start,
        brush(0.0),
        paint(&f, BLUE),
    )
    .expect("begin");
    assert_eq!(
        live.bitmap(),
        None,
        "nothing is placed before the first edit"
    );
    for points in [&[(4.0, 24.0), (30.0, 24.0)][..], &[(60.0, 24.0)][..]] {
        live.extend(&line(points));
        let edit = live.edit(f.session.document(), f.scene).expect("edit");
        f.session.apply(edit.commands.clone()).expect("apply");
        live.shows(&edit);
        assert_eq!(live.at(), edit.at);
    }
    assert_eq!(f.session.document().bitmaps().len(), 1);
    let span = find_span(f.session.document(), f.scene, live.at()).expect("span");
    assert_eq!(span.element_count(), 1);
    assert_eq!(
        pixel(&painted(&f), 56, 24),
        [0, 0, 255, 255],
        "the second stretch"
    );
}

#[test]
fn a_layer_that_cannot_be_drawn_on_is_refused() {
    let mut f = stage();
    for (layer, refusal) in [
        (
            Layer::new(LayerId::new(), "Locked", vec![])
                .expect("layer")
                .with_locked(true),
            DrawRefusal::Locked,
        ),
        (
            Layer::new(LayerId::new(), "Hidden", vec![])
                .expect("layer")
                .with_visible(false),
            DrawRefusal::Hidden,
        ),
    ] {
        let id = layer.id();
        f.session
            .apply([Command::AddLayer {
                scene: f.scene,
                layer,
            }])
            .expect("layer");
        let doc = f.session.document();
        assert_eq!(
            check_paint(doc, f.scene, Target::Layer(id), 0),
            Err(refusal.clone())
        );
        let target = RasterPaint {
            layer: Target::Layer(id),
            ..paint(&f, BLUE)
        };
        let result = lay_paint(doc, f.scene, 0, brush(0.5), target, &line(&[(4.0, 4.0)]));
        assert_eq!(result, Err(BlendError::Refused(refusal)));
    }
    assert_eq!(
        check_paint(f.session.document(), f.scene, Target::Layer(f.layer), 0),
        Ok(())
    );
}

#[test]
fn what_it_cannot_do_is_refused() {
    let f = stage();
    let doc = f.session.document();
    let attempt = |settings: BlendSettings, points: &[(f64, f64)]| {
        lay_paint(doc, f.scene, 0, settings, paint(&f, BLUE), &line(points))
    };
    let on = [(10.0, 10.0)];
    assert_eq!(attempt(brush(1.5), &on), Err(BlendError::Mix));
    assert_eq!(attempt(brush(-0.1), &on), Err(BlendError::Mix));
    assert_eq!(attempt(brush(0.5), &[]), Err(BlendError::NoPoints));
    // Clear paint on a clear bitmap changes nothing, so nothing is placed.
    let nothing = lay_paint(
        doc,
        f.scene,
        0,
        brush(0.5),
        paint(&f, Rgba8::TRANSPARENT),
        &line(&on),
    );
    assert_eq!(nothing, Err(BlendError::NothingChanged));
    // The Raster Brush cannot start as Blur and Smudge do: it needs its paint.
    let start = Pos2::new(10.0, 10.0);
    let begun = BlendStroke::begin(doc, f.scene, 0, start, BlendKind::RasterBrush, brush(0.5));
    assert!(matches!(begun, Err(BlendError::NeedsPaint)));
}

#[test]
fn a_stage_too_large_for_a_bitmap_has_nothing_to_paint_on() {
    let mut f = fixture();
    let stage = Stage::new(8193, 16, Rgba8::opaque(255, 255, 255)).expect("stage");
    f.session.apply([Command::SetStage(stage)]).expect("stage");
    let result = lay_paint(
        f.session.document(),
        f.scene,
        0,
        brush(0.5),
        paint(&f, BLUE),
        &line(&[(10.0, 10.0)]),
    );
    assert_eq!(result, Err(BlendError::StageTooLarge(8193, 16)));
}

#[test]
fn the_picked_up_color_counts_for_as_much_paint_as_it_holds() {
    let blue = [0.0, 0.0, 1.0, 1.0];
    let red = [1.0, 0.0, 0.0, 1.0];
    let mut plain = Painting::new(BLUE);
    assert_eq!(plain.next_color(Some(red), 0.0, 0.5), blue, "no mix");
    let mut over_clear = Painting::new(BLUE);
    assert_eq!(
        over_clear.next_color(Some([0.0; 4]), 1.0, 0.5),
        blue,
        "clear gives none"
    );
    let mut over_red = Painting::new(BLUE);
    assert_eq!(
        over_red.next_color(Some(red), 1.0, 0.5),
        red,
        "all mix takes all of it"
    );
    // Half-clear red counts for half as much.
    let mut faint = Painting::new(BLUE);
    let half_red = [0.5, 0.0, 0.0, 0.5];
    assert_eq!(
        faint.next_color(Some(half_red), 1.0, 0.5),
        [0.5, 0.0, 0.5, 1.0]
    );
    // Off the bitmap the brush keeps what it carried.
    assert_eq!(over_red.next_color(None, 1.0, 0.5), red);
}
