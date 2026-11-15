//! The Pencil under a pen: through the window's own events, a pen line follows its pressure and
//! is kept as a filled outline in the stroke color, while a mouse line, a dashed line and a line
//! drawn with pressure turned off stay plain strokes of one width.

use super::*;
use flare_document::StrokeStyle;

/// Drags the Pencil level across the stage while the native pen source says `pressure`, and
/// returns the line it committed.
fn pencil_line(ws: &mut Workspace, pressure: Option<f32>) -> Shape {
    ws.set_tool(Tool::Pencil);
    ws.paint.stroke_width = 6.0;
    let feed = ws.pen_feed();
    let ctx = egui::Context::default();
    run(ws, &ctx, 0.0);
    run(ws, &ctx, 0.1);
    let button = |at, pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let start = Pos2::new(500.0, 330.0);
    pass(ws, &ctx, input(0.2, vec![Event::PointerMoved(start)]));
    feed.set(pressure);
    pass(ws, &ctx, input(0.3, vec![button(start, true)]));
    let mut time = 0.3;
    let mut at = start;
    for step in 1..=10 {
        time += 0.02;
        at = Pos2::new(500.0 + 20.0 * step as f32, 330.0);
        pass(ws, &ctx, input(time, vec![Event::PointerMoved(at)]));
    }
    pass(ws, &ctx, input(time + 0.02, vec![button(at, false)]));
    assert_eq!(drawing_count(ws), 1, "the line was committed");
    drawn_shape(ws, 1)
}

fn height(shape: &Shape) -> f32 {
    ops::shape_bounds(shape).expect("bounds").height()
}

#[test]
fn a_pen_pencil_line_is_a_filled_outline_in_the_stroke_color_that_thins_when_light() {
    let mut ws = workspace();
    let color = ws.paint.stroke_color.to_array();
    let light = pencil_line(&mut ws, Some(0.1));
    assert!(light.stroke().is_none(), "kept as an outline");
    assert_eq!(
        light.fill(),
        flare_core::Rgba8::new(color[0], color[1], color[2], color[3])
    );
    let firm = pencil_line(&mut workspace(), Some(0.95));
    assert!(firm.stroke().is_none());
    assert!(
        height(&light) < height(&firm) * 0.5,
        "{} against {}",
        height(&light),
        height(&firm)
    );
    // Pressed nearly fully it is nearly as wide as the stroke width.
    assert!(
        height(&firm) > 5.0 && height(&firm) <= 6.0 + 1e-3,
        "{}",
        height(&firm)
    );
}

#[test]
fn a_pen_held_hard_all_the_way_draws_the_same_plain_stroke_as_a_mouse() {
    let full = pencil_line(&mut workspace(), Some(1.0));
    assert_eq!(full.stroke().map(|s| s.width()), Some(6.0));
}

#[test]
fn a_mouse_line_a_dashed_line_and_a_line_with_pressure_off_stay_strokes() {
    let mouse = pencil_line(&mut workspace(), None);
    assert_eq!(mouse.stroke().map(|s| s.width()), Some(6.0));

    let mut ws = workspace();
    ws.pencil_pressure = flare_tools::pencil::PencilPressure::new(false, 0.1);
    let off = pencil_line(&mut ws, Some(0.2));
    assert_eq!(off.stroke().map(|s| s.width()), Some(6.0));

    let mut ws = workspace();
    ws.paint.stroke_style = StrokeStyle::ALL
        .into_iter()
        .find(|s| *s != StrokeStyle::Solid)
        .expect("a style that is not solid");
    let dashed = pencil_line(&mut ws, Some(0.2));
    assert!(dashed.stroke().is_some());
}

#[test]
fn the_pencil_preview_under_a_pen_is_the_outline_it_commits() {
    let mut ws = workspace();
    ws.set_tool(Tool::Pencil);
    for i in 0..=40 {
        let pressure = 0.2 + 0.02 * i as f32;
        let sample = flare_input::InputSample::new(
            f64::from(i) * 4.0,
            50.0,
            pressure,
            flare_input::PointerKind::Pen,
        )
        .expect("sample");
        ws.push_freehand(sample, 1.0);
    }
    let preview = ws.freehand_preview(1.0);
    assert_eq!(preview.len(), 1);
    let sides = preview[0]
        .sides
        .as_ref()
        .expect("the outline, not a ribbon");
    let width = |i: usize| (sides.left[i] - sides.right[i]).length();
    assert!(width(0) < width(sides.left.len() - 1), "thin where light");
}

#[test]
fn the_pencil_pressure_settings_are_kept_with_the_preferences() {
    let mut ws = workspace();
    ws.pencil_pressure = flare_tools::pencil::PencilPressure::new(false, 0.33);
    let kept = ws.preferences();
    assert!(!kept.pencil_pressure);
    let mut fresh = workspace();
    fresh.apply_preferences(&kept);
    assert_eq!(fresh.pencil_pressure, ws.pencil_pressure);
}
