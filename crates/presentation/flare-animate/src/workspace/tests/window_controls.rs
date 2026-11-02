//! The window's own buttons in the top bar, for a window the system gives no title bar.

use super::layer_names::{click, settle};
use super::*;

/// The middle of button `index` (0 minimize, 1 maximize or restore, 2 close) in the 1280 wide
/// test screen: each is 40 wide, the last at the right edge, and the bar is 28 high.
fn button(index: usize) -> Pos2 {
    Pos2::new(1280.0 - 40.0 * (3.0 - index as f32) + 20.0, 12.0)
}

fn with_controls(on: bool) -> (egui::Context, Workspace, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    ws.set_window_controls(on);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    (ctx, ws, time)
}

#[test]
fn the_minimize_button_asks_the_window_to_minimize() {
    let (ctx, mut ws, mut time) = with_controls(true);
    click(
        &mut ws,
        &ctx,
        &mut time,
        button(0),
        egui::PointerButton::Primary,
    );
    assert_eq!(ws.take_window_request(), Some(WindowRequest::Minimize));
    assert!(!ws.take_quit_request());
}

#[test]
fn the_middle_button_asks_for_maximize_and_restore_the_same_way() {
    let (ctx, mut ws, mut time) = with_controls(true);
    click(
        &mut ws,
        &ctx,
        &mut time,
        button(1),
        egui::PointerButton::Primary,
    );
    assert_eq!(
        ws.take_window_request(),
        Some(WindowRequest::ToggleMaximize)
    );
    ws.set_window_maximized(true);
    settle(&mut ws, &ctx, &mut time);
    click(
        &mut ws,
        &ctx,
        &mut time,
        button(1),
        egui::PointerButton::Primary,
    );
    assert_eq!(
        ws.take_window_request(),
        Some(WindowRequest::ToggleMaximize)
    );
}

#[test]
fn the_close_button_quits_through_the_same_road_as_the_menu() {
    let (ctx, mut ws, mut time) = with_controls(true);
    click(
        &mut ws,
        &ctx,
        &mut time,
        button(2),
        egui::PointerButton::Primary,
    );
    assert!(
        ws.take_quit_request(),
        "so unsaved work is asked about first"
    );
    assert_eq!(ws.take_window_request(), None);
}

#[test]
fn without_them_that_corner_of_the_bar_does_nothing() {
    let (ctx, mut ws, mut time) = with_controls(false);
    for index in 0..3 {
        click(
            &mut ws,
            &ctx,
            &mut time,
            button(index),
            egui::PointerButton::Primary,
        );
    }
    assert!(!ws.take_quit_request());
    assert!(ws.take_window_request().is_none(), "a click is not a drag");
}

#[test]
fn a_new_document_in_the_window_keeps_the_buttons() {
    let (ctx, mut ws, mut time) = with_controls(true);
    ws.new_document_in_place();
    settle(&mut ws, &ctx, &mut time);
    click(
        &mut ws,
        &ctx,
        &mut time,
        button(0),
        egui::PointerButton::Primary,
    );
    assert_eq!(ws.take_window_request(), Some(WindowRequest::Minimize));
}
