//! The Polygon tool: clicked corners become one shape in the paint in hand.

use super::*;

fn click_all(ws: &mut Workspace, corners: &[(f32, f32)]) {
    for &(x, y) in corners {
        ws.polygon_click(Pos2::new(x, y), 1.0);
    }
}

/// The corners of the one shape drawn, in order.
fn drawn_corners(ws: &Workspace) -> Vec<(f64, f64)> {
    let Some(Content::Shape(shape)) = drawings(ws).last().cloned() else {
        panic!("a shape was drawn: {:?}", drawings(ws));
    };
    shape
        .path()
        .iter()
        .filter_map(|s| match s {
            flare_document::PathSegment::MoveTo(p) | flare_document::PathSegment::LineTo(p) => {
                Some((p.as_vec().x, p.as_vec().y))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn clicked_corners_close_on_the_first_into_one_shape_as_one_undo_step() {
    let mut ws = workspace();
    ws.set_tool(Tool::Polygon);
    let before = drawing_count(&ws);
    click_all(
        &mut ws,
        &[(10.0, 10.0), (110.0, 20.0), (90.0, 90.0), (20.0, 70.0)],
    );
    assert_eq!(ws.lasso.len(), 4);
    assert_eq!(
        drawing_count(&ws),
        before,
        "nothing is drawn until it closes"
    );
    // The second click of a double click adds no corner.
    ws.polygon_click(Pos2::new(20.0, 70.0), 1.0);
    assert_eq!(ws.lasso.len(), 4);
    // A click on the first corner closes it.
    ws.polygon_click(Pos2::new(12.0, 11.0), 1.0);
    assert!(ws.lasso.is_empty());
    assert_eq!(drawing_count(&ws), before + 1);
    assert_eq!(
        drawn_corners(&ws),
        vec![(10.0, 10.0), (110.0, 20.0), (90.0, 90.0), (20.0, 70.0)]
    );
    ws.undo();
    assert_eq!(drawing_count(&ws), before);
}

#[test]
fn enter_closes_it_in_the_fill_and_stroke_in_hand() {
    let mut ws = workspace();
    ws.set_tool(Tool::Polygon);
    ws.paint.fill = egui::Color32::from_rgb(200, 30, 40);
    ws.paint.stroke_enabled = true;
    ws.paint.stroke_width = 5.0;
    click_all(&mut ws, &[(0.0, 0.0), (60.0, 0.0), (30.0, 50.0)]);
    ws.polygon_finish();
    let Some(Content::Shape(shape)) = drawings(&ws).last().cloned() else {
        panic!("a triangle");
    };
    assert_eq!(shape.fill(), flare_core::Rgba8::new(200, 30, 40, 255));
    assert_eq!(shape.stroke().map(|s| s.width()), Some(5.0));
}

#[test]
fn fewer_than_three_corners_draw_nothing_and_escape_or_another_tool_drops_the_outline() {
    let mut ws = workspace();
    ws.set_tool(Tool::Polygon);
    let before = ws.document().clone();
    click_all(&mut ws, &[(0.0, 0.0), (50.0, 50.0)]);
    ws.polygon_finish();
    assert!(ws.lasso.is_empty());
    assert_eq!(ws.document(), &before);
    // Corners on one line enclose nothing: said, not drawn.
    click_all(&mut ws, &[(0.0, 0.0), (50.0, 50.0), (100.0, 100.0)]);
    ws.polygon_finish();
    assert_eq!(ws.document(), &before);
    assert!(
        ws.status().is_some_and(|s| s.contains("enclose nothing")),
        "{:?}",
        ws.status()
    );
    click_all(&mut ws, &[(0.0, 0.0), (50.0, 0.0)]);
    ws.cancel_drag();
    assert!(ws.lasso.is_empty(), "Escape drops it");
    click_all(&mut ws, &[(0.0, 0.0), (50.0, 0.0)]);
    ws.set_tool(Tool::Select);
    assert!(ws.lasso.is_empty(), "another tool drops it");
    // Enter with another tool does nothing to the document.
    ws.polygon_finish();
    assert_eq!(ws.document(), &before);
}

#[test]
fn the_polygon_tool_is_in_the_rectangle_slot_and_draws_from_clicks_on_the_stage() {
    let mut ws = workspace();
    assert!(ws.is_enabled(MenuAction::ToolPolygon));
    ws.perform(MenuAction::ToolPolygon);
    assert_eq!(ws.tool, Tool::Polygon);
    assert_eq!(ws.label(MenuAction::ToolPolygon), "Polygon");
    // Clicks on the stage, through the window, far enough apart not to be a double click,
    // then Enter.
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    for _ in 0..3 {
        time += 0.02;
        run(&mut ws, &ctx, time);
    }
    let before = drawing_count(&ws);
    for (x, y) in [(200.0, 200.0), (600.0, 220.0), (400.0, 500.0)] {
        let at = ws
            .stage_view
            .to_screen(ws.stage_panel, ws.stage_extent(), Pos2::new(x, y));
        time += 1.0;
        for event in [
            Event::PointerMoved(at),
            Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
            Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ] {
            time += 0.02;
            pass(&mut ws, &ctx, input(time, vec![event]));
        }
    }
    assert_eq!(ws.lasso.len(), 3, "three corners clicked on the stage");
    time += 1.0;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::Enter, Modifiers::NONE)]),
    );
    assert_eq!(drawing_count(&ws), before + 1);
}
