//! The frame in which the pointer is let go of a brush stroke.

use super::*;

/// Whether `shape` is a mesh in `color`, as the preview and the held stroke are.
fn is_mesh_of(shape: &egui::epaint::Shape, color: Color32) -> bool {
    matches!(shape, egui::epaint::Shape::Mesh(m) if m.vertices.first().is_some_and(|v| v.color == color))
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
    assert_eq!(layer_count(&ws), 2, "the stroke was committed");
    assert!(
        released.shapes.iter().any(|s| is_mesh_of(&s.shape, fill)),
        "something of the stroke is drawn in the frame it is let go, so it does not blink"
    );
}
