//! Renaming a layer by double clicking its name, and the menu on a right click.

use super::*;
use egui::PointerButton;

/// One pass at `time`; returns where the text `wanted` was drawn, if it was.
pub(super) fn find(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: f64,
    events: Vec<Event>,
    wanted: &str,
) -> Option<Pos2> {
    let mut out = ctx.run_ui(input(time, events), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes.iter().rev().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == wanted => {
            Some(t.visual_bounding_rect().center())
        }
        _ => None,
    })
}

pub(super) fn press(at: Pos2, button: PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// A workspace that has drawn a few passes, with the name of its first layer found.
fn settled() -> (egui::Context, Workspace, Pos2, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    rect(&mut ws);
    let mut time = 0.0;
    let mut at = None;
    for _ in 0..4 {
        time += 0.1;
        at = find(&mut ws, &ctx, time, vec![], "Layer 1").or(at);
    }
    (ctx, ws, at.expect("the layer's name is drawn"), time)
}

pub(super) fn click(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: &mut f64,
    at: Pos2,
    button: PointerButton,
) {
    *time += 0.05;
    pass(ws, ctx, input(*time, vec![Event::PointerMoved(at)]));
    *time += 0.05;
    pass(ws, ctx, input(*time, vec![press(at, button, true)]));
    *time += 0.05;
    pass(ws, ctx, input(*time, vec![press(at, button, false)]));
}

pub(super) fn settle(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64) {
    for _ in 0..3 {
        *time += 0.1;
        run(ws, ctx, *time);
    }
}

fn name_of(ws: &Workspace, layer: usize) -> String {
    ws.document().scenes()[0].timeline().layers()[layer]
        .name()
        .to_owned()
}

#[test]
fn double_clicking_a_layer_name_edits_it_in_place_and_enter_keeps_the_new_name() {
    let (ctx, mut ws, at, mut time) = settled();
    click(&mut ws, &ctx, &mut time, at, PointerButton::Primary);
    assert!(ws.layer_renaming.is_none(), "one click only selects");
    click(&mut ws, &ctx, &mut time, at, PointerButton::Primary);
    settle(&mut ws, &ctx, &mut time);
    let (_, text) = ws
        .layer_renaming
        .as_mut()
        .expect("the name is being edited");
    assert_eq!(text, "Layer 1");
    *text = "Hero".to_owned();
    time += 0.1;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::Enter, Modifiers::NONE)]),
    );
    settle(&mut ws, &ctx, &mut time);
    assert!(ws.layer_renaming.is_none());
    assert_eq!(name_of(&ws, 0), "Hero");
}

#[test]
fn escape_while_renaming_keeps_the_old_name_and_an_empty_name_is_refused() {
    let (ctx, mut ws, at, mut time) = settled();
    for _ in 0..2 {
        click(&mut ws, &ctx, &mut time, at, PointerButton::Primary);
    }
    settle(&mut ws, &ctx, &mut time);
    ws.layer_renaming.as_mut().expect("editing").1 = "Nope".to_owned();
    time += 0.1;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    assert!(ws.layer_renaming.is_none());
    assert_eq!(name_of(&ws, 0), "Layer 1");

    let id = ws.document().scenes()[0].timeline().layers()[0].id();
    ws.begin_layer_rename(id);
    assert!(ws.layer_renaming.is_some(), "begun");
    time += 0.1;
    run(&mut ws, &ctx, time);
    assert!(ws.layer_renaming.is_some(), "one pass");
    settle(&mut ws, &ctx, &mut time);
    ws.layer_renaming.as_mut().expect("editing").1 = "  ".to_owned();
    time += 0.1;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::Enter, Modifiers::NONE)]),
    );
    assert_eq!(name_of(&ws, 0), "Layer 1", "an empty name changes nothing");
    assert!(ws.status().is_some_and(|s| s.contains("needs a name")));
}

#[test]
fn a_right_click_on_a_layer_name_opens_a_menu_whose_rename_starts_editing() {
    let (ctx, mut ws, at, mut time) = settled();
    click(&mut ws, &ctx, &mut time, at, PointerButton::Secondary);
    let mut rename = None;
    for _ in 0..4 {
        time += 0.1;
        rename = find(&mut ws, &ctx, time, vec![], "Rename").or(rename);
    }
    let rename = rename.expect("the menu offers Rename");
    for wanted in ["Delete", "Lock", "Hide", "Move up"] {
        time += 0.1;
        assert!(
            find(&mut ws, &ctx, time, vec![], wanted).is_some(),
            "{wanted} is in the menu"
        );
    }
    click(&mut ws, &ctx, &mut time, rename, PointerButton::Primary);
    settle(&mut ws, &ctx, &mut time);
    assert!(ws.layer_renaming.is_some(), "Rename starts the edit");
}

#[test]
fn a_right_click_on_the_frames_opens_no_layer_menu() {
    let (ctx, mut ws, at, mut time) = settled();
    click(
        &mut ws,
        &ctx,
        &mut time,
        at + Vec2::new(400.0, 0.0),
        PointerButton::Secondary,
    );
    let mut seen = false;
    for _ in 0..4 {
        time += 0.1;
        seen |= find(&mut ws, &ctx, time, vec![], "Rename").is_some();
    }
    assert!(!seen);
}

#[test]
fn the_toolbar_no_longer_has_a_rename_box() {
    let (ctx, mut ws, _, mut time) = settled();
    time += 0.1;
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    assert!(!out.shapes.iter().any(|s| matches!(&s.shape,
            egui::epaint::Shape::Text(t) if t.galley.text() == "Rename layer")));
}
