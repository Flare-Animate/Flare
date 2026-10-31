use super::*;
use crate::workspace::panels::color_wells::{Metrics, rects_with};

fn well_rects(origin: Pos2) -> crate::workspace::panels::color_wells::WellRects {
    rects_with(origin, Metrics::FULL)
}
use crate::workspace::panels::tool_slots::{SLOT_GROUPS, Slot, front};

#[test]
fn every_tool_is_in_exactly_one_slot_and_the_strip_has_categories() {
    let all = [
        Tool::Select,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Line,
        Tool::Pen,
        Tool::Text,
        Tool::Hand,
        Tool::Zoom,
        Tool::Eyedropper,
        Tool::Bucket,
        Tool::InkBottle,
        Tool::Eraser,
        Tool::PolyStar,
        Tool::Lasso,
        Tool::Subselection,
        Tool::FreeTransform,
        Tool::Pencil,
        Tool::Brush,
        Tool::Ruler,
        Tool::Mirror,
    ];
    let listed: Vec<Tool> = SLOT_GROUPS
        .iter()
        .flat_map(|group| group.iter())
        .flat_map(|slot| slot.iter().filter_map(|v| v.tool))
        .collect();
    for tool in all {
        assert_eq!(listed.iter().filter(|t| **t == tool).count(), 1, "{tool:?}");
    }
    assert_eq!(listed.len(), all.len());
    assert!(
        SLOT_GROUPS.len() >= 4,
        "the strip is divided into categories"
    );
    let slots: Vec<Slot> = SLOT_GROUPS.iter().flat_map(|g| g.iter().copied()).collect();
    assert!(slots.iter().all(|slot| !slot.is_empty()));
    let names: Vec<&str> = slots
        .iter()
        .flat_map(|s| s.iter().map(|v| v.name))
        .collect();
    for (i, name) in names.iter().enumerate() {
        assert!(!names[..i].contains(name), "{name} is listed twice");
    }
    for slot in &slots {
        for v in slot.iter() {
            assert_eq!(
                v.key.is_empty(),
                v.tool.is_none(),
                "{}: a key only for a built tool",
                v.name
            );
        }
    }
}

#[test]
fn a_slot_shows_the_tool_in_hand_then_the_one_picked_then_the_first_built() {
    let slot = SLOT_GROUPS[1][5];
    assert_eq!(slot[0].name, "Rectangle");
    assert_eq!(front(slot, Tool::Pen, None).name, "Rectangle");
    assert_eq!(front(slot, Tool::Pen, Some(1)).name, "Oval");
    assert_eq!(front(slot, Tool::Ellipse, None).name, "Oval");
    assert_eq!(front(slot, Tool::PolyStar, Some(1)).name, "PolyStar");
    // A choice of a tool that is not built is ignored.
    assert_eq!(front(slot, Tool::Pen, Some(2)).name, "Rectangle");
    // A slot with nothing built shows its first and cannot be picked.
    let three_d = SLOT_GROUPS[0][2];
    assert_eq!(three_d[0].name, "3D Rotation");
    assert!(front(three_d, Tool::Pen, None).tool.is_none());
}

#[test]
fn the_color_box_overlaps_the_squares_but_not_the_buttons() {
    let r = well_rects(Pos2::new(10.0, 20.0));
    assert!(r.fill.intersects(r.stroke), "the stroke sits over the fill");
    assert!(r.stroke.min.x > r.fill.min.x && r.stroke.min.y > r.fill.min.y);
    for button in [r.swap, r.defaults] {
        assert!(!button.intersects(r.fill) && !button.intersects(r.stroke));
    }
    assert!(!r.swap.intersects(r.defaults));
}

#[test]
fn the_tool_strip_draws_at_narrow_and_wide_widths() {
    for width in [30.0, 36.0, 60.0, 120.0, 600.0] {
        let mut ws = workspace();
        ws.paint.stroke_color = Color32::from_rgb(1, 2, 3);
        let ctx = egui::Context::default();
        let raw = RawInput {
            time: Some(0.0),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(width, 700.0))),
            ..RawInput::default()
        };
        let mut output = ctx.run_ui(raw, |ui| ws.tools_panel(ui));
        output.textures_delta.clear();
        assert_eq!(
            ws.tool(),
            Tool::Rectangle,
            "drawing the strip changes nothing"
        );
    }
}

#[test]
fn the_color_box_fits_inside_the_narrowest_tool_strip_and_its_own_height() {
    use crate::workspace::panels::TOOL_ICON;
    use crate::workspace::panels::color_wells::{BOX, OFFSET, SQUARE};
    let narrowest = (TOOL_ICON + 6.0) * 2.0 + 3.0;
    assert!(
        OFFSET + SQUARE <= narrowest,
        "{} against {narrowest}",
        OFFSET + SQUARE
    );
    let r = well_rects(Pos2::ZERO);
    for rect in [r.fill, r.stroke, r.swap, r.defaults] {
        assert!(
            rect.max.y <= BOX && rect.max.x <= OFFSET + SQUARE,
            "{rect:?}"
        );
    }
}

/// The lowest edge of anything the strip drew, and what was drawn there.
fn lowest_edge(ws: &mut Workspace, ctx: &egui::Context, raw: RawInput) -> f32 {
    let mut output = ctx.run_ui(raw, |ui| ws.tools_panel(ui));
    output.textures_delta.clear();
    output
        .shapes
        .iter()
        .map(|s| s.shape.visual_bounding_rect().max.y)
        .filter(|y| y.is_finite())
        .fold(0.0, f32::max)
}

#[test]
fn a_tool_strip_taller_than_its_panel_scrolls_so_the_color_box_can_be_reached() {
    // One narrow column and a short window: the tools and the color box do not fit.
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(60.0, 300.0));
    let raw = |time, events| RawInput {
        time: Some(time),
        screen_rect: Some(screen),
        events,
        ..RawInput::default()
    };
    let mut ws = workspace();
    let ctx = egui::Context::default();
    let inside = Pos2::new(30.0, 150.0);
    // Two passes to settle the layout, with the pointer over the strip.
    lowest_edge(&mut ws, &ctx, raw(0.0, vec![Event::PointerMoved(inside)]));
    let before = lowest_edge(&mut ws, &ctx, raw(0.1, vec![]));
    assert!(
        before > screen.height(),
        "the strip is taller than its panel: {before}"
    );
    let wheel = Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: Vec2::new(0.0, -2000.0),
        phase: egui::TouchPhase::Move,
        source: egui::MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    };
    lowest_edge(&mut ws, &ctx, raw(0.2, vec![wheel]));
    // The scroll eases in over a few passes.
    let mut after = 0.0;
    for pass in 1..=8 {
        after = lowest_edge(&mut ws, &ctx, raw(0.2 + f64::from(pass) * 0.1, vec![]));
    }
    assert!(
        after <= screen.height() + 8.0,
        "scrolled to the end, the color box is on screen: {after} against {}",
        screen.height()
    );
}

#[test]
fn in_a_strip_one_tool_wide_the_color_box_is_the_compact_one_and_it_fits() {
    use crate::workspace::panels::TOOL_ICON;
    let cell = TOOL_ICON + 6.0;
    let wide = (TOOL_ICON + 6.0) * 2.0 + 3.0;
    // A one-tool strip is a cell and its margins across, a few pixels more than the cell.
    assert!(
        Metrics::for_width(cell).width() <= cell + 4.0,
        "the compact box is {} wide for a {cell} cell",
        Metrics::for_width(cell).width()
    );
    assert_eq!(
        Metrics::for_width(wide + 20.0).width(),
        Metrics::FULL.width()
    );
    let m = Metrics::COMPACT;
    let r = rects_with(Pos2::ZERO, m);
    let all = [r.fill, r.stroke, r.swap, r.defaults];
    for rect in all {
        assert!(
            rect.min.x >= 0.0 && rect.max.x <= m.width() && rect.max.y <= m.height(),
            "{rect:?} is outside the {} x {} box",
            m.width(),
            m.height()
        );
    }
    // The two squares overlap on purpose; the buttons touch neither square nor each other.
    for button in [r.swap, r.defaults] {
        assert!(!button.intersects(r.fill) && !button.intersects(r.stroke));
    }
    assert!(!r.swap.intersects(r.defaults));
    assert!(r.stroke.min.x > r.fill.min.x && r.stroke.min.y > r.fill.min.y);
}

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
    let mut out = ctx.run_ui(raw, |ui| picked = ws.slot_cell(ui, SLOT_GROUPS[1][5]));
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
fn subselection_is_in_the_selection_slot_and_the_strip_has_animates_other_tools() {
    let slots: Vec<Slot> = SLOT_GROUPS.iter().flat_map(|g| g.iter().copied()).collect();
    let selection = slots
        .iter()
        .find(|s| s[0].name == "Selection")
        .expect("slot");
    assert_eq!(selection[1].name, "Subselection");
    assert_eq!(selection[1].tool, Some(Tool::Subselection));
    // With Subselection in hand, its own slot shows it, and picking it elsewhere does not move it.
    assert_eq!(
        front(selection, Tool::Subselection, None).name,
        "Subselection"
    );
    assert_eq!(front(selection, Tool::Select, None).name, "Selection");
    let names: Vec<&str> = slots
        .iter()
        .flat_map(|s| s.iter().map(|v| v.name))
        .collect();
    for name in [
        "Asset Warp",
        "Rotate View",
        "Gradient Transform",
        "Ink Bottle",
        "Oval",
    ] {
        assert!(names.contains(&name), "{name} is in the strip's lists");
    }
    // Those not built are marked so, never selectable.
    for slot in &slots {
        for v in slot.iter() {
            if ["Asset Warp", "Rotate View"].contains(&v.name) {
                assert!(v.tool.is_none(), "{} is not built", v.name);
            }
        }
    }
}
