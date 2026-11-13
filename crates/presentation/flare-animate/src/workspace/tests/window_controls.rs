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

/// What the top bar shows with the pointer at `x` in a window with its own buttons.
fn hint_at(x: f32) -> String {
    let (ctx, mut ws, _) = with_controls(true);
    let at = Pos2::new(x, 12.0);
    let mut text = String::new();
    for step in 0..10 {
        let events = if step == 0 {
            vec![Event::PointerMoved(at)]
        } else {
            vec![]
        };
        let mut out = ctx.run_ui(input(5.0 + f64::from(step) * 0.2, events), |ui| ws.show(ui));
        out.textures_delta.clear();
        for shape in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &shape.shape {
                text.push_str(t.galley.text());
                text.push('|');
            }
        }
    }
    text
}

#[test]
fn the_house_is_at_the_far_left_and_play_publish_and_the_switcher_hug_the_window_buttons() {
    // The house at the left edge of the bar: pressing it there goes to Home.
    let (ctx, mut ws, mut time) = with_controls(true);
    assert!(!ws.home_shown());
    click(
        &mut ws,
        &ctx,
        &mut time,
        Pos2::new(18.0, 14.0),
        egui::PointerButton::Primary,
    );
    assert!(ws.home_shown(), "the house is at the far left");
    // The window buttons take the last 120 px; the three buttons sit right against them.
    let (mut play, mut publish, mut switcher) = (None, None, None);
    let mut x = 1156.0;
    while x > 1040.0 {
        let hint = hint_at(x);
        if hint.contains("Workspaces") {
            switcher.get_or_insert(x);
        }
        if hint.contains("Publish") {
            publish.get_or_insert(x);
        }
        if hint.contains("Play") {
            play.get_or_insert(x);
        }
        x -= 6.0;
    }
    let (play, publish, switcher) = (
        play.expect("play"),
        publish.expect("publish"),
        switcher.expect("switcher"),
    );
    assert!(
        play < publish && publish < switcher,
        "{play} {publish} {switcher}"
    );
    assert!(
        switcher > 1120.0,
        "the switcher is against the minimize button: {switcher}"
    );
}

#[test]
fn the_menus_sit_inside_the_bar_whether_or_not_the_window_has_its_own_buttons() {
    // The bar is 28 px high with its own buttons and 38 without; a button placed instead of
    // painted pushed the menus 20 px below it.
    for (controls, bar) in [(true, 28.0), (false, 38.0)] {
        let (ctx, mut ws, _) = with_controls(controls);
        let mut out = ctx.run_ui(input(9.0, vec![]), |ui| ws.show(ui));
        out.textures_delta.clear();
        let file = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == "File" => {
                    Some(t.visual_bounding_rect())
                }
                _ => None,
            })
            .expect("the File menu is drawn");
        assert!(
            file.min.y >= 0.0 && file.max.y <= bar,
            "controls {controls}: File at {file:?} is not inside a bar {bar} high"
        );
    }
}
