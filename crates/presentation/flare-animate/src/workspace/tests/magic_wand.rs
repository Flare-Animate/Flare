//! The Magic Wand in the workspace: clicks pick pixels of a bitmap through the pixels job,
//! Shift and Alt add and take away, Delete clears them as one undo step, and what is picked
//! goes when it no longer fits the document.

use super::*;
use flare_core::{BitmapId, InstanceId, Transform2D};
use flare_document::{Bitmap, BitmapInstance};

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// A 4x2 picture, white on the left half and black on the right, drawn ten times its size with
/// its corner at (100, 100), on frames 0 to 23.
fn picture_workspace() -> (Workspace, BitmapId) {
    let id = BitmapId::new();
    let rgba = [WHITE, WHITE, BLACK, BLACK].repeat(2).concat();
    let bitmap = Bitmap::new(id, "picture", 4, 2, rgba).expect("bitmap");
    let matrix =
        DAffine2::from_scale_angle_translation(DVec2::splat(10.0), 0.0, DVec2::new(100.0, 100.0));
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
    let mut ws = Workspace::new(document, None);
    ws.set_tool(Tool::MagicWand);
    (ws, id)
}

const ON_WHITE: Pos2 = Pos2::new(105.0, 105.0);
const ON_BLACK: Pos2 = Pos2::new(135.0, 115.0);

fn picked(ws: &Workspace) -> usize {
    ws.wand
        .picked
        .as_ref()
        .map_or(0, |p| p.selection.mask.count())
}

fn shown_bitmap(ws: &Workspace) -> BitmapId {
    match &ws.document().scenes()[0].timeline().layers()[0].spans()[0].content() {
        Content::Bitmap(placed) => placed.bitmap(),
        other => panic!("not a bitmap: {other:?}"),
    }
}

fn alpha_at(ws: &Workspace, x: usize, y: usize) -> u8 {
    let bitmap = ws.document().bitmap(shown_bitmap(ws)).expect("bitmap");
    bitmap.pixels()[(y * 4 + x) * 4 + 3]
}

#[test]
fn clicks_pick_add_and_take_away_and_invert_flips_what_is_picked() {
    let (mut ws, _) = picture_workspace();
    ws.wand_click(ON_WHITE, false, false);
    assert_eq!(picked(&ws), 4, "the white half");
    ws.wand_click(ON_BLACK, true, false);
    assert_eq!(picked(&ws), 8, "Shift adds the black half");
    ws.wand_click(ON_WHITE, false, true);
    assert_eq!(picked(&ws), 4, "Alt takes the white half away");
    ws.wand_invert();
    assert_eq!(picked(&ws), 4);
    assert!(
        ws.wand
            .picked
            .as_ref()
            .expect("picked")
            .selection
            .mask
            .contains(0, 0)
    );
    assert!(
        !ws.has_selection(),
        "picked pixels are not a selected element"
    );
}

#[test]
fn delete_clears_the_picked_pixels_as_one_undo_step() {
    let (mut ws, original) = picture_workspace();
    let ctx = egui::Context::default();
    ws.wand.settings.smoothing = flare_tools::magic_wand::Smoothing::Pixels;
    ws.wand_click(ON_WHITE, false, false);
    pass(
        &mut ws,
        &ctx,
        input(0.0, vec![key(Key::Delete, Modifiers::NONE)]),
    );
    assert_ne!(shown_bitmap(&ws), original);
    assert_eq!((alpha_at(&ws, 0, 0), alpha_at(&ws, 3, 1)), (0, 255));
    assert!(ws.wand.picked.is_none(), "nothing is left picked");
    assert!(
        ws.document().bitmap(original).is_none(),
        "the old picture is not kept"
    );
    ws.perform(MenuAction::Undo);
    assert_eq!(shown_bitmap(&ws), original);
}

#[test]
fn the_edit_menu_deletes_and_deselects_picked_pixels() {
    let (mut ws, original) = picture_workspace();
    assert!(!ws.is_enabled(MenuAction::Delete));
    ws.wand_click(ON_BLACK, false, false);
    assert!(ws.is_enabled(MenuAction::Delete) && ws.is_enabled(MenuAction::DeselectAll));
    ws.perform(MenuAction::DeselectAll);
    assert!(ws.wand.picked.is_none());
    ws.wand_click(ON_BLACK, false, false);
    ws.perform(MenuAction::Delete);
    assert_ne!(shown_bitmap(&ws), original);
}

#[test]
fn a_click_off_the_bitmap_says_why_and_escape_and_another_tool_let_go() {
    let (mut ws, _) = picture_workspace();
    ws.wand_click(Pos2::new(10.0, 10.0), false, false);
    assert!(ws.wand.picked.is_none());
    assert!(
        ws.status.as_deref().is_some_and(|s| s.contains("bitmap")),
        "{:?}",
        ws.status
    );
    let ctx = egui::Context::default();
    ws.wand_click(ON_WHITE, false, false);
    pass(
        &mut ws,
        &ctx,
        input(0.0, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    assert!(ws.wand.picked.is_none());
    ws.wand_click(ON_WHITE, false, false);
    ws.set_tool(Tool::Select);
    assert!(ws.wand.picked.is_none());
}

#[test]
fn picked_pixels_go_when_their_bitmap_leaves_the_stage() {
    let (mut ws, _) = picture_workspace();
    ws.wand_click(ON_WHITE, false, false);
    ws.wand_check();
    assert!(ws.wand.picked.is_some(), "still there");
    let layer = ws.document().scenes()[0].timeline().layers()[0].id();
    let scene = ws.document().scenes()[0].id();
    ws.run_batch(
        vec![Command::RemoveSpan {
            scene,
            layer,
            start: 0,
        }],
        "delete",
    );
    ws.wand_check();
    assert!(
        ws.wand.picked.is_none(),
        "the keyframe that showed it is gone"
    );
}

#[test]
fn a_clear_made_for_an_older_document_changes_nothing() {
    let (mut ws, original) = picture_workspace();
    ws.wand_click(ON_WHITE, false, false);
    let older = ws.active().snapshot();
    ws.run_batch(
        vec![Command::SetFrameRate(FrameRate::new(12, 1).expect("rate"))],
        "change the frame rate",
    );
    ws.wand.pending = Some(99);
    ws.wand_cleared_arrived(99, older, Ok(vec![]));
    assert_eq!(shown_bitmap(&ws), original);
    assert!(ws.wand.pending.is_none());
    assert!(ws.status.is_some(), "it says nothing was cleared");
}

#[test]
fn the_stage_draws_the_picked_pixels() {
    let (mut ws, _) = picture_workspace();
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    ws.wand_click(ON_WHITE, false, false);
    for step in 0..3 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| ws.show(ui));
        out.textures_delta.clear();
    }
    assert!(ws.wand.texture_made(), "the stripes were made to be drawn");
}
