//! The tool strip's slots: the lists that open from a cell, and picking from them.

use super::*;
use crate::workspace::panels::tool_slots::SLOT_GROUPS;

/// One pass of a lone Rectangle-slot cell at the top left of a 300 x 300 window: the tool it
/// picked, and where each piece of text was drawn.
fn slot_pass(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: f64,
    events: Vec<Event>,
) -> (Option<Tool>, Vec<(String, Pos2)>) {
    let raw = RawInput {
        time: Some(time),
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0))),
        events,
        ..RawInput::default()
    };
    let mut picked = None;
    let mut out = ctx.run_ui(raw, |ui| picked = ws.slot_cell(ui, SLOT_GROUPS[1][3]));
    out.textures_delta.clear();
    let texts = out
        .shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some((
                t.galley.text().to_owned(),
                t.visual_bounding_rect().center(),
            )),
            _ => None,
        })
        .collect();
    (picked, texts)
}

fn button(at: Pos2, which: egui::PointerButton, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: which,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn a_right_click_opens_the_slots_list_and_picking_from_it_changes_what_the_cell_shows() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let cell = Pos2::new(12.0, 12.0);
    let secondary = egui::PointerButton::Secondary;
    slot_pass(&mut ws, &ctx, 0.0, vec![Event::PointerMoved(cell)]);
    let (_, closed) = slot_pass(&mut ws, &ctx, 0.1, vec![]);
    assert!(
        !closed.iter().any(|(t, _)| t == "Oval"),
        "the list starts shut"
    );
    slot_pass(&mut ws, &ctx, 0.2, vec![button(cell, secondary, true)]);
    slot_pass(&mut ws, &ctx, 0.3, vec![button(cell, secondary, false)]);
    slot_pass(&mut ws, &ctx, 0.4, vec![]);
    let (_, open) = slot_pass(&mut ws, &ctx, 0.5, vec![]);
    let names: Vec<&str> = open.iter().map(|(t, _)| t.as_str()).collect();
    for name in [
        "Rectangle",
        "Oval",
        "Rectangle Primitive",
        "Oval Primitive",
        "PolyStar",
    ] {
        assert!(names.contains(&name), "{name} is in the list: {names:?}");
    }
    // The tool that is not built cannot be picked.
    let primitive = open
        .iter()
        .find(|(t, _)| t == "Rectangle Primitive")
        .expect("row")
        .1;
    let primary = egui::PointerButton::Primary;
    slot_pass(&mut ws, &ctx, 0.6, vec![Event::PointerMoved(primitive)]);
    slot_pass(&mut ws, &ctx, 0.7, vec![button(primitive, primary, true)]);
    let (picked, _) = slot_pass(&mut ws, &ctx, 0.8, vec![button(primitive, primary, false)]);
    assert_eq!(
        picked, None,
        "a tool that is not built is dimmed and does nothing"
    );
    // A built one is picked and the cell then shows it.
    let oval = open.iter().find(|(t, _)| t == "Oval").expect("row").1;
    slot_pass(&mut ws, &ctx, 0.9, vec![Event::PointerMoved(oval)]);
    slot_pass(&mut ws, &ctx, 1.0, vec![button(oval, primary, true)]);
    let (picked, _) = slot_pass(&mut ws, &ctx, 1.1, vec![button(oval, primary, false)]);
    assert_eq!(picked, Some(Tool::Ellipse));
    assert_eq!(
        ws.slot_choice.get("Rectangle"),
        Some(&1),
        "and it is remembered"
    );
}

#[test]
fn a_long_press_opens_the_list_and_escape_shuts_it() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let cell = Pos2::new(12.0, 12.0);
    let primary = egui::PointerButton::Primary;
    slot_pass(&mut ws, &ctx, 0.0, vec![Event::PointerMoved(cell)]);
    slot_pass(&mut ws, &ctx, 0.1, vec![]);
    slot_pass(&mut ws, &ctx, 0.2, vec![button(cell, primary, true)]);
    slot_pass(&mut ws, &ctx, 0.3, vec![]);
    let (_, short) = slot_pass(&mut ws, &ctx, 0.4, vec![]);
    assert!(
        !short.iter().any(|(t, _)| t == "Oval"),
        "a short press opens nothing"
    );
    // Held past the delay, still down.
    slot_pass(&mut ws, &ctx, 0.7, vec![]);
    slot_pass(&mut ws, &ctx, 0.8, vec![]);
    let (_, held) = slot_pass(&mut ws, &ctx, 0.9, vec![]);
    assert!(
        held.iter().any(|(t, _)| t == "Oval"),
        "the list is open: {held:?}"
    );
    let escape = key(Key::Escape, Modifiers::NONE);
    slot_pass(
        &mut ws,
        &ctx,
        1.0,
        vec![button(cell, primary, false), escape],
    );
    slot_pass(&mut ws, &ctx, 1.1, vec![]);
    let (_, after) = slot_pass(&mut ws, &ctx, 1.2, vec![]);
    assert!(!after.iter().any(|(t, _)| t == "Oval"), "Escape shuts it");
}

#[test]
fn the_dots_cell_lists_the_other_tools_and_picking_one_selects_it() {
    use crate::workspace::panels::tool_slots::EXTRA;
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let pass_with = |ws: &mut Workspace, time: f64, events: Vec<Event>| {
        let raw = RawInput {
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0))),
            events,
            ..RawInput::default()
        };
        let mut picked = None;
        let mut out = ctx.run_ui(raw, |ui| picked = ws.slot_cell(ui, EXTRA));
        out.textures_delta.clear();
        let texts: Vec<(String, Pos2)> = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some((
                    t.galley.text().to_owned(),
                    t.visual_bounding_rect().center(),
                )),
                _ => None,
            })
            .collect();
        (picked, texts)
    };
    let cell = Pos2::new(12.0, 12.0);
    let primary = egui::PointerButton::Primary;
    pass_with(&mut ws, 0.0, vec![Event::PointerMoved(cell)]);
    pass_with(&mut ws, 0.1, vec![]);
    pass_with(&mut ws, 0.2, vec![button(cell, primary, true)]);
    pass_with(&mut ws, 0.3, vec![button(cell, primary, false)]);
    pass_with(&mut ws, 0.4, vec![]);
    let (_, open) = pass_with(&mut ws, 0.5, vec![]);
    let names: Vec<&str> = open.iter().map(|(t, _)| t.as_str()).collect();
    for name in ["Mirror", "Ruler", "3D Rotation", "Deco", "Bone", "Camera"] {
        assert!(names.contains(&name), "{name} is listed: {names:?}");
    }
    let ruler = open.iter().find(|(t, _)| t == "Ruler").expect("row").1;
    pass_with(&mut ws, 0.6, vec![Event::PointerMoved(ruler)]);
    pass_with(&mut ws, 0.7, vec![button(ruler, primary, true)]);
    let (picked, _) = pass_with(&mut ws, 0.8, vec![button(ruler, primary, false)]);
    assert_eq!(
        picked,
        Some(Tool::Ruler),
        "a built tool is picked from the dots"
    );
}

#[test]
fn the_dots_cell_says_more_tools_when_hovered() {
    use crate::workspace::panels::tool_slots::EXTRA;
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let cell = Pos2::new(12.0, 12.0);
    let mut seen = Vec::new();
    for step in 0..14 {
        let raw = RawInput {
            time: Some(f64::from(step) * 0.1),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(300.0, 300.0))),
            events: if step == 0 {
                vec![Event::PointerMoved(cell)]
            } else {
                vec![]
            },
            ..RawInput::default()
        };
        let mut out = ctx.run_ui(raw, |ui| {
            ws.slot_cell(ui, EXTRA);
        });
        out.textures_delta.clear();
        seen = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
    }
    assert!(seen.iter().any(|t| t == "More tools"), "{seen:?}");
    assert!(!seen.iter().any(|t| t.contains("Edit toolbar")), "{seen:?}");
}
