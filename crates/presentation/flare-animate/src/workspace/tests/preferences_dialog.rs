//! Preferences is a dialog in the middle of the window.

use super::*;

/// Where the text `wanted` was drawn in a pass with `events`.
fn text_at(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: f64,
    events: Vec<Event>,
    wanted: &str,
) -> Option<Pos2> {
    let mut out = ctx.run_ui(input(time, events), |ui| ws.show(ui));
    out.textures_delta.clear();
    // The dialog is drawn last, over anything of the same name behind its backdrop.
    out.shapes.iter().rev().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == wanted => {
            Some(t.visual_bounding_rect().center())
        }
        _ => None,
    })
}

fn opened() -> (Workspace, egui::Context) {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    ws.windows.preferences = true;
    // The dialog takes a few passes to find its size.
    for pass_number in 1..=4 {
        run(&mut ws, &ctx, 0.05 * f64::from(pass_number));
    }
    (ws, ctx)
}

#[test]
fn preferences_opens_in_the_middle_of_the_window() {
    let (mut ws, ctx) = opened();
    let title = text_at(&mut ws, &ctx, 0.2, vec![], "Preferences").expect("the title is drawn");
    // The window is 1280 x 800 and the dialog's content 360 wide, so the heading, which starts
    // the content, begins 180 left of the middle; the dialog sits in the middle band, not at an
    // edge.
    assert!(
        title.x > 460.0 && title.x < 560.0,
        "centered across: {title:?}"
    );
    assert!(
        title.y > 100.0 && title.y < 400.0,
        "in the middle band: {title:?}"
    );
}

/// Clicks at `at` in three passes.
fn click_at(ws: &mut Workspace, ctx: &egui::Context, time: f64, at: Pos2) {
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(ws, ctx, input(time, vec![Event::PointerMoved(at)]));
    pass(ws, ctx, input(time + 0.1, vec![click(true)]));
    pass(ws, ctx, input(time + 0.2, vec![click(false)]));
    run(ws, ctx, time + 0.3);
}

#[test]
fn escape_the_close_button_and_the_x_all_close_it() {
    let (mut ws, ctx) = opened();
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    run(&mut ws, &ctx, 0.3);
    assert!(!ws.windows.preferences, "Escape closes it");

    let (mut ws, ctx) = opened();
    let close = text_at(&mut ws, &ctx, 0.2, vec![], "Close").expect("the button is drawn");
    click_at(&mut ws, &ctx, 0.3, close);
    assert!(!ws.windows.preferences, "Close closes it");

    // The X is a picture, so it is found from the title: same row, at the content's right edge.
    let (mut ws, ctx) = opened();
    let title = text_at(&mut ws, &ctx, 0.2, vec![], "Preferences").expect("the title is drawn");
    let x = Pos2::new(640.0 + 180.0 - 10.0, title.y);
    click_at(&mut ws, &ctx, 0.3, x);
    assert!(!ws.windows.preferences, "the X at the top right closes it");
}

#[test]
fn close_is_at_the_bottom_right_and_restore_defaults_is_red_at_the_left() {
    let (mut ws, ctx) = opened();
    let close = text_at(&mut ws, &ctx, 0.2, vec![], "Close").expect("close");
    let restore = text_at(&mut ws, &ctx, 0.3, vec![], "Restore defaults").expect("restore");
    let title = text_at(&mut ws, &ctx, 0.4, vec![], "Preferences").expect("title");
    assert!(close.x > 640.0, "Close is on the right half: {close:?}");
    assert!(
        restore.x < 640.0,
        "Restore defaults is on the left: {restore:?}"
    );
    assert!(
        (close.y - restore.y).abs() < 4.0 && close.y > title.y + 100.0,
        "both are on the last row, below everything else"
    );
    // The button behind its label is red.
    let mut out = ctx.run_ui(input(0.5, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    let red = out.shapes.iter().any(|s| match &s.shape {
        egui::epaint::Shape::Rect(r) => {
            r.rect.contains(restore) && r.fill.r() > 150 && r.fill.g() < 90 && r.fill.b() < 90
        }
        _ => false,
    });
    assert!(red, "Restore defaults sits on a red button");
}
