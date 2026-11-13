//! Blur and Smudge in the workspace: a drag over a bitmap is laid by the pixels worker and shown as
//! it goes, the whole drag one undo step; a cancelled drag leaves nothing, and a drag off any
//! bitmap says so.

use super::*;
use flare_core::{BitmapId, InstanceId, Transform2D};
use flare_document::{Bitmap, BitmapInstance};
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

fn at(x: f64, y: f64) -> InputSample {
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
