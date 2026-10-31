//! Tools dragged from the "…" list into the strip, kept, and taken out again.

use super::*;
use crate::workspace::preferences::Preferences;
use egui::PointerButton;

fn press(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// One whole-window pass; where the text `wanted` was drawn, if it was.
fn find(
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

/// A window with the "…" list open, and where the row called `name` is.
fn open_list(name: &str) -> (egui::Context, Workspace, Pos2, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    run(&mut ws, &ctx, 0.0);
    ws.slot_open = Some("Mirror");
    let mut row = None;
    let mut time = 0.0;
    for _ in 0..4 {
        time += 0.1;
        row = find(&mut ws, &ctx, time, vec![], name).or(row);
    }
    (ctx, ws, row.expect("the list has the row"), time)
}

/// Drags from `from` to `to` and lets go.
fn drag(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, from: Pos2, to: Pos2) {
    let mut step = |events: Vec<Event>| {
        *time += 0.05;
        pass(ws, ctx, input(*time, events));
    };
    step(vec![Event::PointerMoved(from)]);
    step(vec![press(from, true)]);
    step(vec![Event::PointerMoved(from + Vec2::new(12.0, 0.0))]);
    step(vec![Event::PointerMoved(to)]);
    step(vec![Event::PointerMoved(to)]);
    step(vec![press(to, false)]);
    step(vec![]);
}

#[test]
fn dragging_a_tool_from_the_list_onto_the_strip_puts_it_there_and_out_of_the_list() {
    let (ctx, mut ws, row, mut time) = open_list("Ruler");
    assert!(ws.pinned_tools.is_empty());
    let before = ws.tool;
    drag(&mut ws, &ctx, &mut time, row, Pos2::new(12.0, 400.0));
    assert_eq!(ws.pinned_tools, vec!["Ruler"]);
    assert_eq!(ws.pinned_slots().len(), 1);
    assert!(ws.slot_drag.is_none(), "the drag is over");
    // Opened again, the list no longer holds it.
    ws.slot_open = Some("Mirror");
    let mut still = false;
    for _ in 0..4 {
        time += 0.1;
        still |= find(&mut ws, &ctx, time, vec![], "Ruler").is_some();
    }
    assert!(!still, "a pinned tool leaves the list");
    assert_eq!(ws.tool, before, "dragging does not pick the tool");
}

#[test]
fn letting_go_away_from_the_strip_or_pressing_escape_pins_nothing() {
    let (ctx, mut ws, row, mut time) = open_list("Ruler");
    drag(&mut ws, &ctx, &mut time, row, Pos2::new(700.0, 400.0));
    assert!(ws.pinned_tools.is_empty());
    assert!(ws.slot_drag.is_none());

    ws.slot_open = Some("Mirror");
    let mut row = None;
    for _ in 0..4 {
        time += 0.1;
        row = find(&mut ws, &ctx, time, vec![], "Ruler").or(row);
    }
    let row = row.expect("row");
    let mut step = |ws: &mut Workspace, events: Vec<Event>| {
        time += 0.05;
        pass(ws, &ctx, input(time, events));
    };
    step(&mut ws, vec![Event::PointerMoved(row)]);
    step(&mut ws, vec![press(row, true)]);
    step(
        &mut ws,
        vec![Event::PointerMoved(row + Vec2::new(12.0, 0.0))],
    );
    step(&mut ws, vec![Event::PointerMoved(Pos2::new(12.0, 400.0))]);
    assert!(ws.slot_drag.is_some(), "being dragged");
    step(&mut ws, vec![key(Key::Escape, Modifiers::NONE)]);
    step(&mut ws, vec![press(Pos2::new(12.0, 400.0), false)]);
    assert!(ws.pinned_tools.is_empty(), "Escape drops it");
}

#[test]
fn a_tool_not_built_can_be_pinned_too_and_each_is_pinned_once() {
    let mut ws = workspace();
    assert!(ws.pin_tool("Deco"), "Animate lets any of them be placed");
    assert!(!ws.pin_tool("Deco"), "once");
    assert!(!ws.pin_tool("Pencil"), "only the list's tools");
    assert!(!ws.pin_tool("nothing"));
    assert_eq!(ws.pinned_tools, vec!["Deco"]);
    ws.unpin_tool("Deco");
    assert!(ws.pinned_tools.is_empty());
}

#[test]
fn pinned_tools_are_kept_with_the_preferences_and_a_bad_file_cannot_add_others() {
    let mut ws = workspace();
    ws.pin_tool("Ruler");
    ws.pin_tool("Camera");
    let saved = ws.preferences();
    assert_eq!(saved.pinned_tools, vec!["Ruler", "Camera"]);
    let mut fresh = workspace();
    fresh.apply_preferences(&saved);
    assert_eq!(fresh.pinned_tools, vec!["Ruler", "Camera"]);
    let hostile = Preferences {
        pinned_tools: vec![
            "Ruler".into(),
            "Ruler".into(),
            "Selection".into(),
            "x".repeat(10_000),
        ],
        ..Preferences::default()
    };
    fresh.apply_preferences(&hostile);
    assert_eq!(fresh.pinned_tools, vec!["Ruler"]);
    fresh.apply_preferences(&Preferences::default());
    assert!(
        fresh.pinned_tools.is_empty(),
        "restoring defaults empties the strip's extras"
    );
}

#[test]
fn a_right_click_on_a_pinned_tool_offers_to_take_it_out() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    ws.pin_tool("Ruler");
    let slot = ws.pinned_slots()[0];
    let pass_cell = |ws: &mut Workspace, time: f64, events: Vec<Event>| {
        let raw = RawInput {
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0))),
            events,
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            ws.slot_cell(ui, slot);
        });
        out.textures_delta.clear();
        out.shapes.iter().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == "Remove from toolbar" => {
                Some(t.visual_bounding_rect().center())
            }
            _ => None,
        })
    };
    let cell = Pos2::new(12.0, 12.0);
    let secondary = |pressed| Event::PointerButton {
        pos: cell,
        button: PointerButton::Secondary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass_cell(&mut ws, 0.0, vec![Event::PointerMoved(cell)]);
    pass_cell(&mut ws, 0.1, vec![secondary(true)]);
    pass_cell(&mut ws, 0.2, vec![secondary(false)]);
    let mut item = None;
    for i in 0..4 {
        item = pass_cell(&mut ws, 0.3 + f64::from(i) * 0.1, vec![]).or(item);
    }
    let item = item.expect("the menu offers it");
    pass_cell(&mut ws, 1.0, vec![Event::PointerMoved(item)]);
    pass_cell(&mut ws, 1.1, vec![press(item, true)]);
    pass_cell(&mut ws, 1.2, vec![press(item, false)]);
    assert!(ws.pinned_tools.is_empty(), "removed");
}
