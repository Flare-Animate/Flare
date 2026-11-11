//! The frame in which the pointer is let go of a brush stroke.

use super::*;

/// Whether `shape` is a mesh painted in `color`, as the preview and the held stroke are (its
/// edges fade out to clear, so only the inside is in the color itself).
fn is_mesh_of(shape: &egui::epaint::Shape, color: Color32) -> bool {
    matches!(shape, egui::epaint::Shape::Mesh(m) if m.vertices.iter().any(|v| v.color == color))
}

#[test]
fn the_stroke_is_on_screen_in_the_frame_the_pointer_is_released() {
    let mut ws = workspace();
    ws.set_tool(Tool::Brush);
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let button = |at, pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    let start = Pos2::new(600.0, 380.0);
    pass(&mut ws, &ctx, input(0.2, vec![Event::PointerMoved(start)]));
    pass(&mut ws, &ctx, input(0.3, vec![button(start, true)]));
    let mut time = 0.3;
    let mut at = start;
    for step in 1..=8 {
        time += 0.02;
        at = Pos2::new(600.0 + 12.0 * step as f32, 380.0 + (step % 3) as f32 * 9.0);
        pass(&mut ws, &ctx, input(time, vec![Event::PointerMoved(at)]));
    }
    assert!(ws.freehand.len() > 1, "the drag was taken as a stroke");
    let fill = ws.paint.fill;
    let mut released = ctx.run_ui(input(time + 0.02, vec![button(at, false)]), |ui| {
        ws.show(ui)
    });
    released.textures_delta.clear();
    assert_eq!(drawing_count(&ws), 1, "the stroke was committed");
    assert!(
        released.shapes.iter().any(|s| is_mesh_of(&s.shape, fill)),
        "something of the stroke is drawn in the frame it is let go, so it does not blink"
    );
}

/// Drags with `tool` across the stage and returns the shapes drawn in the frame the pointer is
/// let go, with the paint in use.
fn released_drag(tool: Tool) -> (Vec<egui::epaint::Shape>, Color32, Color32, usize) {
    let mut ws = workspace();
    ws.set_tool(tool);
    ws.paint.fill = Color32::from_rgb(210, 40, 40);
    ws.paint.stroke_color = Color32::from_rgb(20, 20, 220);
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
    pass(&mut ws, &ctx, input(0.3, vec![button(start, true)]));
    let mut time = 0.3;
    let mut at = start;
    for step in 1..=6 {
        time += 0.02;
        at = Pos2::new(500.0 + 30.0 * step as f32, 330.0 + 20.0 * step as f32);
        pass(&mut ws, &ctx, input(time, vec![Event::PointerMoved(at)]));
    }
    let (fill, stroke) = (ws.paint.fill, ws.paint.stroke_color);
    let mut released = ctx.run_ui(input(time + 0.02, vec![button(at, false)]), |ui| {
        ws.show(ui)
    });
    released.textures_delta.clear();
    let layers = drawing_count(&ws);
    (
        released.shapes.iter().map(|s| s.shape.clone()).collect(),
        fill,
        stroke,
        layers,
    )
}

#[test]
fn a_rectangle_and_an_oval_are_on_screen_in_the_frame_the_pointer_is_released() {
    let (shapes, fill, _, layers) = released_drag(Tool::Rectangle);
    assert_eq!(layers, 1, "the rectangle was committed");
    assert!(
        shapes
            .iter()
            .any(|s| matches!(s, egui::epaint::Shape::Rect(r) if r.fill == fill)),
        "the rectangle is drawn in the frame it is let go, in its own color and not as the half-seen preview"
    );
    let (shapes, fill, _, layers) = released_drag(Tool::Ellipse);
    assert_eq!(layers, 1);
    assert!(
        shapes
            .iter()
            .any(|s| matches!(s, egui::epaint::Shape::Ellipse(e) if e.fill == fill)),
        "the oval is drawn in the frame it is let go"
    );
}

#[test]
fn a_line_is_on_screen_in_the_frame_the_pointer_is_released() {
    let (shapes, _, stroke, layers) = released_drag(Tool::Line);
    assert_eq!(layers, 1, "the line was committed");
    assert!(
        shapes.iter().any(|s| is_mesh_of(s, stroke)),
        "the line is drawn in the frame it is let go, so it does not flash"
    );
}
