//! Blur, Smudge and the Raster Brush in the workspace: a drag is laid by the pixels worker and
//! shown as it goes, the whole drag one undo step; a cancelled drag leaves nothing, and a drag off
//! any bitmap (Blur, Smudge) or on a layer that cannot be drawn on (the Raster Brush) says so. The
//! Raster Brush paints the fill color into a new picture on an empty layer, and a second drag
//! paints into the same picture and mixes with it. Modify > Convert to Bitmap turns a selected
//! shape into pixels the Raster Brush then mixes with.

use super::*;
use flare_core::{BitmapId, InstanceId, Transform2D};
use flare_document::{Bitmap, BitmapInstance, Stage};
use flare_input::InputSample;

/// One-pixel stripes of black and white, 100 pixels square, drawn thirty times its size from
/// (-1000, -1000), so it covers the stage and everything round it; on frames 0 to 23.
fn striped_workspace() -> (Workspace, BitmapId) {
    let id = BitmapId::new();
    let rgba = (0..100 * 100)
        .flat_map(|i| {
            if i % 2 == 0 {
                [0u8, 0, 0, 255]
            } else {
                [255, 255, 255, 255]
            }
        })
        .collect();
    let bitmap = Bitmap::new(id, "stripes", 100, 100, rgba).expect("bitmap");
    let matrix = DAffine2::from_scale_angle_translation(
        DVec2::splat(30.0),
        0.0,
        DVec2::new(-1000.0, -1000.0),
    );
    let placed = BitmapInstance::new(
        InstanceId::new(),
        id,
        Transform2D::new(matrix).expect("matrix"),
    );
    let span = FrameSpan::new(0, NonZeroU32::new(24).expect("24"), Content::Bitmap(placed));
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("document")
        .with_bitmaps(vec![bitmap])
        .expect("bitmaps");
    (Workspace::new(document, None), id)
}

fn shown_bitmap(ws: &Workspace) -> BitmapId {
    match &ws.document().scenes()[0].timeline().layers()[0].spans()[0].content() {
        Content::Bitmap(placed) => placed.bitmap(),
        other => panic!("not a bitmap: {other:?}"),
    }
}

fn button(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

pub(super) fn at(x: f64, y: f64) -> InputSample {
    InputSample::mouse(x, y).expect("sample")
}

#[test]
fn a_blur_drag_shows_as_it_goes_and_undoes_in_one_step() {
    let (mut ws, original) = striped_workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    ws.set_tool(Tool::Blur);
    let before = ws.document().clone();
    let steps = ws.active().undo_labels().count();
    let (from, to) = (Pos2::new(560.0, 340.0), Pos2::new(660.0, 360.0));
    let mut time = 0.2;
    pass(&mut ws, &ctx, input(time, vec![Event::PointerMoved(from)]));
    time += 0.1;
    pass(&mut ws, &ctx, input(time, vec![button(from, true)]));
    for step in 1..=10 {
        time += 0.02;
        let point = from + (to - from) * (step as f32 / 10.0);
        pass(&mut ws, &ctx, input(time, vec![Event::PointerMoved(point)]));
    }
    assert_ne!(
        shown_bitmap(&ws),
        original,
        "blurred before the pen is lifted"
    );
    assert!(
        ws.freehand_preview(1.0).is_empty(),
        "no stroke is drawn over the picture: the picture itself shows it"
    );
    pass(&mut ws, &ctx, input(time + 0.02, vec![button(to, false)]));
    assert_eq!(ws.active().undo_labels().count(), steps + 1, "one step");
    assert_eq!(ws.active().undo_labels().last(), Some("Blur"));
    assert_eq!(ws.document().bitmaps().len(), 1, "no picture left over");
    ws.undo();
    assert_eq!(ws.document(), &before, "one undo puts it back");
}

#[test]
fn a_smudge_drag_carries_on_to_where_it_is_let_go() {
    let (mut ws, original) = striped_workspace();
    ws.set_tool(Tool::Smudge);
    ws.freehand = vec![at(100.0, 100.0)];
    ws.blend_as_dragged();
    assert_eq!(
        shown_bitmap(&ws),
        original,
        "one dab has only picked color up"
    );
    ws.freehand.push(at(400.0, 120.0));
    ws.finish_freehand(1.0);
    assert_ne!(shown_bitmap(&ws), original, "laid on release");
    assert_eq!(ws.active().undo_labels().last(), Some("Smudge"));
    assert!(!ws.paint.blend.stroking(), "the stroke is over");
}

#[test]
fn a_blur_click_softens_a_spot_and_a_smudge_click_changes_nothing() {
    let (mut ws, original) = striped_workspace();
    ws.set_tool(Tool::Smudge);
    ws.click_with_tool(Pos2::new(100.0, 100.0), false);
    assert_eq!(shown_bitmap(&ws), original);
    assert!(ws.status.is_none(), "nothing to say about it");
    ws.set_tool(Tool::Blur);
    ws.click_with_tool(Pos2::new(100.0, 100.0), false);
    assert_ne!(shown_bitmap(&ws), original);
}

#[test]
fn cancelling_a_drag_takes_back_what_it_showed() {
    let (mut ws, _) = striped_workspace();
    ws.set_tool(Tool::Blur);
    let before = ws.document().clone();
    ws.freehand = vec![at(100.0, 100.0), at(300.0, 100.0)];
    ws.blend_as_dragged();
    assert_ne!(ws.document(), &before, "shown while dragging");
    ws.cancel_drag();
    assert_eq!(ws.document(), &before, "the cancelled drag leaves nothing");
    assert!(!ws.paint.blend.stroking());
}

#[test]
fn a_drag_off_any_bitmap_says_so() {
    let (mut ws, _) = striped_workspace();
    ws.set_tool(Tool::Blur);
    let before = ws.document().clone();
    ws.freehand = vec![at(5000.0, 5000.0), at(5100.0, 5000.0)];
    ws.blend_as_dragged();
    ws.finish_freehand(1.0);
    assert_eq!(ws.document(), &before);
    assert!(
        ws.status.as_deref().is_some_and(|s| s.contains("bitmap")),
        "{:?}",
        ws.status
    );
}

#[test]
fn each_tool_keeps_its_own_settings() {
    let (mut ws, _) = striped_workspace();
    ws.set_tool(Tool::Smudge);
    assert_eq!(ws.blend_size(), Some(30.0));
    ws.paint.blend.smudge.size = 12.0;
    ws.set_tool(Tool::Blur);
    assert_eq!(ws.blend_size(), Some(80.0));
    ws.set_tool(Tool::Smudge);
    assert_eq!(ws.blend_size(), Some(12.0));
    ws.set_tool(Tool::Eraser);
    assert_eq!(ws.blend_size(), None);
}

/// A 200 by 100 stage with one empty layer.
pub(super) fn empty_workspace() -> Workspace {
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("document")
        .with_stage(Stage::new(200, 100, Rgba8::WHITE).expect("stage"));
    Workspace::new(document, None)
}

/// Drags the tool in hand through `points`, the worker showing it as it goes, and lets go.
pub(super) fn drag(ws: &mut Workspace, points: &[(f64, f64)]) {
    let (first, rest) = points.split_first().expect("a point");
    ws.freehand = vec![at(first.0, first.1)];
    ws.blend_as_dragged();
    for (x, y) in rest {
        ws.freehand.push(at(*x, *y));
        ws.blend_as_dragged();
    }
    ws.finish_freehand(1.0);
}

/// The pictures on the first layer at frame 0, bottom first.
pub(super) fn pictures(ws: &Workspace) -> Vec<Bitmap> {
    let document = ws.document();
    let Some(span) = document.scenes()[0].timeline().layers()[0].span_at(0) else {
        return Vec::new();
    };
    span.elements()
        .filter_map(|content| match content {
            Content::Bitmap(placed) => document.bitmap(placed.bitmap()).cloned(),
            _ => None,
        })
        .collect()
}

/// The Brush on the first layer made a raster layer, as the Raster Brush was before raster layers.
fn raster_brush(ws: &mut Workspace) {
    let layer = ws.document().scenes()[0].timeline().layers()[0].id();
    ws.convert_layer(layer, true);
    ws.set_tool(Tool::Brush);
}

/// The pixel of a new painting under stage point `(x, y)`: it has `PAINT_DENSITY` pixels to a
/// stage unit.
pub(super) fn at_stage(bitmap: &Bitmap, x: u32, y: u32) -> [u8; 4] {
    let d = flare_tools::blend::PAINT_DENSITY;
    pixel(bitmap, x * d, y * d)
}

fn pixel(bitmap: &Bitmap, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * bitmap.width() + x) * 4) as usize;
    let p = &bitmap.pixels()[i..i + 4];
    [p[0], p[1], p[2], p[3]]
}

#[test]
fn a_drag_on_an_empty_layer_paints_the_fill_color_into_a_new_picture_in_one_step() {
    let mut ws = empty_workspace();
    raster_brush(&mut ws);
    ws.paint.fill = Color32::from_rgb(0, 0, 255);
    let before = ws.document().clone();
    let steps = ws.active().undo_labels().count();
    ws.freehand = vec![at(20.0, 50.0), at(100.0, 50.0)];
    ws.blend_as_dragged();
    assert_eq!(pictures(&ws).len(), 1, "painted before the pen is lifted");
    ws.freehand.push(at(180.0, 50.0));
    ws.finish_freehand(1.0);
    assert!(!ws.paint.blend.stroking(), "the stroke is over");
    let painted = pictures(&ws);
    assert_eq!(painted.len(), 1, "one picture, painted on as the drag went");
    let picture = &painted[0];
    let d = flare_tools::blend::PAINT_DENSITY;
    assert_eq!(
        (picture.width(), picture.height()),
        (200 * d, 100 * d),
        "the stage's size, at the painting's density"
    );
    assert_eq!(at_stage(picture, 60, 50), [0, 0, 255, 255]);
    assert_eq!(
        at_stage(picture, 160, 50),
        [0, 0, 255, 255],
        "the last stretch"
    );
    assert_eq!(at_stage(picture, 60, 10), [0, 0, 0, 0]);
    assert_eq!(ws.document().bitmaps().len(), 1, "no picture left over");
    assert_eq!(ws.active().undo_labels().count(), steps + 1, "one step");
    assert_eq!(ws.active().undo_labels().last(), Some("Raster Brush"));
    ws.undo();
    assert_eq!(ws.document(), &before, "one undo puts it back");
}

#[test]
fn a_second_drag_paints_into_the_same_picture_and_mixes_with_the_first() {
    let mut ws = empty_workspace();
    raster_brush(&mut ws);
    ws.paint.blend.raster.size = 30.0;
    ws.paint.fill = Color32::from_rgb(255, 0, 0);
    drag(&mut ws, &[(10.0, 50.0), (190.0, 50.0)]);
    ws.paint.fill = Color32::from_rgb(0, 0, 255);
    ws.paint.blend.raster.mix = 0.5;
    drag(&mut ws, &[(20.0, 50.0), (180.0, 50.0)]);
    let painted = pictures(&ws);
    assert_eq!(painted.len(), 1, "painted into, not stacked on");
    assert_eq!(ws.document().bitmaps().len(), 1);
    let mixed = at_stage(&painted[0], 100, 50);
    assert!(
        (100..=155).contains(&mixed[0]) && (100..=155).contains(&mixed[2]),
        "blue painted half mixed over red: {mixed:?}"
    );
}

#[test]
fn a_locked_layer_is_not_painted_and_says_why() {
    let mut ws = empty_workspace();
    let layer = ws.document().scenes()[0].timeline().layers()[0].id();
    raster_brush(&mut ws);
    ws.toggle_layer_locked(layer);
    let before = ws.document().clone();
    drag(&mut ws, &[(20.0, 50.0), (100.0, 50.0)]);
    assert_eq!(ws.document(), &before);
    assert!(!ws.paint.blend.stroking());
    let message = tr!(ws, "tools-draw-locked", &[("name", "Layer 1")]);
    assert_eq!(ws.status.as_deref(), Some(message.as_str()));
}

#[test]
fn cancelling_a_drag_takes_back_the_new_picture() {
    let mut ws = empty_workspace();
    raster_brush(&mut ws);
    let before = ws.document().clone();
    ws.freehand = vec![at(20.0, 50.0), at(100.0, 50.0)];
    ws.blend_as_dragged();
    assert_ne!(ws.document(), &before, "shown while dragging");
    ws.cancel_drag();
    assert_eq!(ws.document(), &before, "the cancelled drag leaves nothing");
    assert!(!ws.paint.blend.stroking());
}

#[test]
fn the_raster_brush_keeps_its_own_settings() {
    let mut ws = empty_workspace();
    raster_brush(&mut ws);
    assert_eq!(ws.blend_size(), Some(20.0));
    ws.paint.blend.raster.size = 44.0;
    ws.set_tool(Tool::Smudge);
    assert_eq!(ws.blend_size(), Some(30.0));
    ws.set_tool(Tool::Brush);
    assert_eq!(ws.blend_size(), Some(44.0));
}

/// The empty workspace with a red band across the middle of its stage, selected.
fn red_band_selected() -> Workspace {
    let mut ws = empty_workspace();
    let layer = ws.document().scenes()[0].timeline().layers()[0].id();
    let band = Shape::rect(
        ShapeId::new(),
        0.0,
        30.0,
        200.0,
        40.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let scene = ws.scene_id().expect("scene");
    ws.run_batch(
        vec![Command::InsertSpan {
            scene,
            layer,
            span: FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(band)),
        }],
        "draw",
    );
    ws.selected = Some(SpanRef {
        layer,
        start: 0,
        element: 0,
    });
    ws
}

#[test]
fn convert_to_bitmap_turns_a_shape_into_pixels_the_raster_brush_mixes_with() {
    let mut ws = red_band_selected();
    let steps = ws.active().undo_labels().count();
    assert!(ws.is_enabled(MenuAction::ConvertToBitmap));
    ws.perform(MenuAction::ConvertToBitmap);
    assert_eq!(ws.active().undo_labels().count(), steps + 1, "one step");
    assert_eq!(ws.active().undo_labels().last(), Some("Convert to bitmap"));
    let painted = pictures(&ws);
    assert_eq!(painted.len(), 1, "the shape became one picture");
    assert_eq!((painted[0].width(), painted[0].height()), (200, 40));
    assert_eq!(pixel(&painted[0], 100, 20), [255, 0, 0, 255]);
    let selected = ws.selected.expect("the bitmap is selected");
    assert!(matches!(
        ops::find_element(ws.document(), ws.scene_id().expect("scene"), selected),
        Some(Content::Bitmap(_))
    ));
    // The Brush on the layer made raster paints into it and mixes with the red.
    raster_brush(&mut ws);
    ws.paint.blend.raster.size = 30.0;
    ws.paint.blend.raster.mix = 0.5;
    ws.paint.fill = Color32::from_rgb(0, 0, 255);
    drag(&mut ws, &[(20.0, 50.0), (180.0, 50.0)]);
    let painted = pictures(&ws);
    assert_eq!(painted.len(), 1, "painted into the converted picture");
    let mixed = pixel(&painted[0], 100, 20);
    assert!(
        (100..=155).contains(&mixed[0]) && (100..=155).contains(&mixed[2]),
        "blue mixed with the converted red: {mixed:?}"
    );
}

#[test]
fn convert_to_bitmap_needs_a_selection_and_says_why_it_cannot() {
    let mut ws = red_band_selected();
    ws.selected = None;
    assert!(
        !ws.is_enabled(MenuAction::ConvertToBitmap),
        "nothing selected"
    );
    let mut ws = red_band_selected();
    let layer = ws.document().scenes()[0].timeline().layers()[0].id();
    // A selection on two keyframes cannot be one bitmap.
    ws.also_selected = vec![SpanRef {
        layer,
        start: 9,
        element: 0,
    }];
    let before = ws.document().clone();
    ws.perform(MenuAction::ConvertToBitmap);
    assert_eq!(ws.document(), &before);
    assert!(
        ws.status
            .as_deref()
            .is_some_and(|s| s.contains("one keyframe")),
        "{:?}",
        ws.status
    );
}
