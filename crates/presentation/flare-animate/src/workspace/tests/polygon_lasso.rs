//! The polygon lasso: clicked corners, closing the loop, and the bounds on it.

use super::*;

#[test]
fn the_polygon_lasso_collects_clicked_corners_and_selects_what_the_closed_loop_encloses() {
    let mut ws = workspace();
    rect(&mut ws); // centered at (60, 35)
    ws.commit_rectangle(Pos2::new(300.0, 300.0), Pos2::new(340.0, 340.0));
    ws.set_tool(Tool::PolygonLasso);
    for corner in [(0.0, 0.0), (200.0, 0.0), (200.0, 100.0), (0.0, 100.0)] {
        ws.polygon_lasso_click(Pos2::new(corner.0, corner.1), 1.0, false);
    }
    assert_eq!(ws.lasso.len(), 4);
    assert!(
        ws.selection_all().is_empty(),
        "nothing is selected until the loop closes"
    );
    // A repeated click, as the second click of a double click, adds no corner.
    ws.polygon_lasso_click(Pos2::new(0.0, 100.0), 1.0, false);
    assert_eq!(ws.lasso.len(), 4);
    // A click on the first corner closes the loop.
    ws.polygon_lasso_click(Pos2::new(3.0, 2.0), 1.0, false);
    assert!(ws.lasso.is_empty());
    assert_eq!(ws.selection_all().len(), 1);
    // Enter closes it too, and Shift adds to what is selected.
    for corner in [(250.0, 250.0), (400.0, 250.0), (400.0, 400.0)] {
        ws.polygon_lasso_click(Pos2::new(corner.0, corner.1), 1.0, true);
    }
    ws.polygon_lasso_finish(true);
    assert_eq!(ws.selection_all().len(), 2);
}

#[test]
fn a_polygon_lasso_of_fewer_than_three_corners_keeps_the_selection_and_escape_drops_the_loop() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.select_at(Pos2::new(60.0, 35.0), false);
    assert_eq!(ws.selection_all().len(), 1);
    ws.set_tool(Tool::PolygonLasso);
    ws.polygon_lasso_click(Pos2::new(500.0, 500.0), 1.0, false);
    ws.polygon_lasso_click(Pos2::new(600.0, 600.0), 1.0, false);
    ws.polygon_lasso_finish(false);
    assert!(ws.lasso.is_empty());
    assert_eq!(ws.selection_all().len(), 1);
    // Enter with no loop in progress does not clear the selection either.
    ws.polygon_lasso_finish(false);
    assert_eq!(ws.selection_all().len(), 1);
    ws.polygon_lasso_click(Pos2::new(500.0, 500.0), 1.0, false);
    ws.cancel_drag();
    assert!(ws.lasso.is_empty());
    // Corners are bounded.
    for i in 0..5000 {
        ws.polygon_lasso_click(Pos2::new(i as f32 * 20.0, 0.0), 1.0, false);
    }
    assert!(ws.lasso.len() <= 4096);
}
