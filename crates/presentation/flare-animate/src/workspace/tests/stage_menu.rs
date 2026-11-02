use super::layer_names::{find, press};
use super::*;
use egui::PointerButton;

/// A workspace with its rectangle drawn, and the screen point of the middle of that rectangle.
fn drawn() -> (egui::Context, Workspace, egui::Pos2, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    // Well inside the stage, clear of the rulers along its edges.
    ws.commit_rectangle(egui::pos2(600.0, 300.0), egui::pos2(900.0, 500.0));
    let mut time = 0.0;
    for _ in 0..4 {
        time += 0.1;
        run(&mut ws, &ctx, time);
    }
    let extent = ws.stage_extent();
    let image = ws.stage_view.image_rect(ws.stage_panel, extent);
    let scale = ws.stage_view.scale(ws.stage_panel, extent);
    let at = image.min + egui::vec2(750.0, 400.0) * scale;
    (ctx, ws, at, time)
}

fn right_click(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, at: egui::Pos2) {
    for events in [
        vec![Event::PointerMoved(at)],
        vec![press(at, PointerButton::Secondary, true)],
        vec![press(at, PointerButton::Secondary, false)],
    ] {
        *time += 0.05;
        pass(ws, ctx, input(*time, events));
    }
}

#[test]
fn a_right_click_on_a_shape_selects_it_and_opens_the_menu_for_it() {
    let (ctx, mut ws, at, mut time) = drawn();
    assert!(ws.selection_all().is_empty());
    right_click(&mut ws, &ctx, &mut time, at);
    assert_eq!(ws.selection_all().len(), 1, "the shape under the pointer");
    let mut group = None;
    for _ in 0..4 {
        time += 0.1;
        group = find(&mut ws, &ctx, time, vec![], "Group").or(group);
    }
    assert!(group.is_some(), "the menu is open");
    for wanted in ["Cut", "Copy", "Paste in Place", "Convert to Symbol"] {
        time += 0.1;
        assert!(
            find(&mut ws, &ctx, time, vec![], wanted).is_some(),
            "{wanted}"
        );
    }
}

#[test]
fn choosing_delete_in_the_stage_menu_removes_what_was_picked() {
    let (ctx, mut ws, at, mut time) = drawn();
    right_click(&mut ws, &ctx, &mut time, at);
    let mut delete = None;
    for _ in 0..4 {
        time += 0.1;
        delete = find(&mut ws, &ctx, time, vec![], "Delete").or(delete);
    }
    let delete = delete.expect("the menu is open");
    for events in [
        vec![Event::PointerMoved(delete)],
        vec![press(delete, PointerButton::Primary, true)],
        vec![press(delete, PointerButton::Primary, false)],
    ] {
        time += 0.05;
        pass(&mut ws, &ctx, input(time, events));
    }
    assert!(ws.selection_all().is_empty());
    assert_eq!(spans_of(&ws, 1), 0, "the rectangle is gone");
}

#[test]
fn a_right_click_on_empty_stage_clears_the_selection() {
    let (ctx, mut ws, at, mut time) = drawn();
    right_click(&mut ws, &ctx, &mut time, at);
    assert_eq!(ws.selection_all().len(), 1);
    let extent = ws.stage_extent();
    let image = ws.stage_view.image_rect(ws.stage_panel, extent);
    let scale = ws.stage_view.scale(ws.stage_panel, extent);
    let empty = image.min + egui::vec2(200.0, 400.0) * scale;
    // The first click closes the menu that is open, the second is the one on the empty stage.
    right_click(&mut ws, &ctx, &mut time, empty);
    right_click(&mut ws, &ctx, &mut time, empty);
    assert!(ws.selection_all().is_empty());
}
