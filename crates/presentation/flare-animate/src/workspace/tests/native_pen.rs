//! A pen whose pressure comes from a native source beside the window's own events.

use super::*;
use flare_input::PointerKind;

#[test]
fn with_no_pen_the_device_is_a_mouse_at_full_pressure() {
    let ws = Workspace::untitled().expect("workspace");
    assert_eq!(ws.pointer_reading(None), (1.0, PointerKind::Mouse));
}

#[test]
fn a_pen_on_the_surface_reports_its_pressure_over_a_touch_force_and_over_a_mouse() {
    let ws = Workspace::untitled().expect("workspace");
    let feed = ws.pen_feed();
    feed.set(Some(0.3));
    assert_eq!(ws.pointer_reading(None), (0.3, PointerKind::Pen));
    assert_eq!(ws.pointer_reading(Some(0.9)), (0.3, PointerKind::Pen));
    feed.set(None);
    assert_eq!(ws.pointer_reading(None), (1.0, PointerKind::Mouse));
    assert_eq!(
        ws.pointer_reading(Some(0.9)),
        (0.9, PointerKind::Pen),
        "the window's own touch force still works where there is one"
    );
}

/// Drags the Brush level across the stage through the window's own events while the native pen
/// source says `pressure`, and returns how tall the painted stroke is.
fn brush_stroke_height(pressure: Option<f32>) -> f32 {
    let mut ws = workspace();
    ws.set_tool(Tool::Brush);
    let feed = ws.pen_feed();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let button = |at, pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let start = Pos2::new(500.0, 330.0);
    pass(&mut ws, &ctx, input(0.2, vec![Event::PointerMoved(start)]));
    feed.set(pressure);
    pass(&mut ws, &ctx, input(0.3, vec![button(start, true)]));
    let mut time = 0.3;
    let mut at = start;
    for step in 1..=10 {
        time += 0.02;
        at = Pos2::new(500.0 + 20.0 * step as f32, 330.0);
        pass(&mut ws, &ctx, input(time, vec![Event::PointerMoved(at)]));
    }
    pass(&mut ws, &ctx, input(time + 0.02, vec![button(at, false)]));
    assert_eq!(drawing_count(&ws), 1, "the stroke was committed");
    ops::shape_bounds(&drawn_shape(&ws, 1))
        .expect("bounds")
        .height()
}

#[test]
fn a_light_pen_paints_a_thinner_brush_stroke_on_the_stage_than_a_firm_one_or_a_mouse() {
    let (light, firm, mouse) = (
        brush_stroke_height(Some(0.1)),
        brush_stroke_height(Some(1.0)),
        brush_stroke_height(None),
    );
    assert!(light > 0.0 && light < firm * 0.5, "{light} against {firm}");
    assert!((firm - mouse).abs() < 1e-3, "{firm} against {mouse}");
}

#[test]
fn the_pen_still_reaches_the_stage_after_a_new_document_a_new_tab_and_a_tab_switch() {
    let mut ws = Workspace::untitled().expect("workspace");
    // The window takes the feed once, when it opens.
    let feed = ws.pen_feed();
    let mut pressure = 0.2;
    let mut check = |ws: &Workspace, after: &str| {
        pressure += 0.1;
        feed.set(Some(pressure));
        assert_eq!(
            ws.pointer_reading(None),
            (pressure, PointerKind::Pen),
            "{after}"
        );
    };
    check(&ws, "at the start");
    ws.new_document_in_place();
    check(&ws, "after a new document in place");
    let document = ws.new_blank_document().expect("a document");
    ws.install_document(document, None);
    check(&ws, "after a document was installed");
    let document = ws.new_blank_document().expect("a document");
    ws.open_in_new_tab(document, None);
    check(&ws, "after a new tab");
    ws.switch_to_tab(0);
    check(&ws, "after switching tabs");
}
