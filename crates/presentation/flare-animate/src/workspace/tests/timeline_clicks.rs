//! The timeline driven as a person drives it: a click on a cell, then a key.

use super::layer_names::{click, settle};
use super::*;
use egui::PointerButton;

/// The middle of the cell for frame `frame` (zero based) on row `row`, where it was last drawn.
fn cell(ws: &Workspace, row: usize, frame: u32) -> Pos2 {
    let l = ws.timeline_layout.expect("the timeline was drawn");
    Pos2::new(l.frame_x(frame as f32 + 0.5), l.row_top(row) + 10.0)
}

fn drawn() -> (egui::Context, Workspace, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace_with_one_frame());
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    (ctx, ws, time)
}

fn workspace_with_one_frame() -> Workspace {
    let mut ws = blank_workspace();
    rect(&mut ws);
    ws
}

#[test]
fn clicking_an_empty_cell_puts_the_playhead_there_and_f5_holds_the_layer_out_to_it() {
    let (ctx, mut ws, mut time) = drawn();
    let at = cell(&ws, 0, 9);
    click(&mut ws, &ctx, &mut time, at, PointerButton::Primary);
    assert_eq!(ws.frame(), 9, "the playhead is on the clicked cell");
    // A person takes a moment before the key: the playhead must still be there after a few
    // repaints (the frame field once clamped it back to the one-frame scene).
    settle(&mut ws, &ctx, &mut time);
    assert_eq!(ws.frame(), 9, "and stays there");
    time += 0.1;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::F5, Modifiers::NONE)]),
    );
    assert_eq!(ws.status(), None);
    assert_eq!(
        active_spans(&ws),
        vec![(0, 10)],
        "frames up to the clicked one"
    );
}

/// Whether a rectangle of the selected cells' blue was painted around `at`.
fn blue_cell_at(ws: &mut Workspace, ctx: &egui::Context, time: f64, at: Pos2) -> bool {
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes.iter().any(|s| match &s.shape {
        egui::epaint::Shape::Rect(r) => {
            r.stroke.color == crate::workspace::timeline_rows::SELECTED_CELL
                && r.rect.expand(1.0).contains(at)
        }
        _ => false,
    })
}

#[test]
fn a_clicked_empty_cell_is_shown_selected_on_its_layer() {
    let (ctx, mut ws, mut time) = drawn();
    ws.add_layer();
    settle(&mut ws, &ctx, &mut time);
    // Two layers now; the cell at frame 17 of the second row is empty.
    let at = cell(&ws, 1, 16);
    click(&mut ws, &ctx, &mut time, at, PointerButton::Primary);
    let lower = ws.document().scenes()[0].timeline().layers()[1].id();
    assert_eq!(
        ws.active_layer_id(),
        Some(lower),
        "its layer is the active one"
    );
    assert_eq!(ws.frame(), 16);
    time += 0.1;
    assert!(
        blue_cell_at(&mut ws, &ctx, time, at),
        "the cell is drawn selected"
    );
    let elsewhere = cell(&ws, 0, 16);
    time += 0.1;
    assert!(
        !blue_cell_at(&mut ws, &ctx, time, elsewhere),
        "not the other layer's"
    );
}
