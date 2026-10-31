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
    out.shapes.iter().find_map(|s| match &s.shape {
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
    run(&mut ws, &ctx, 0.1);
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

#[test]
fn escape_and_done_close_it_and_it_stays_closed() {
    let (mut ws, ctx) = opened();
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    run(&mut ws, &ctx, 0.3);
    assert!(!ws.windows.preferences, "Escape closes it");

    let (mut ws, ctx) = opened();
    let done = text_at(&mut ws, &ctx, 0.2, vec![], "Done").expect("the button is drawn");
    let click = |pressed| Event::PointerButton {
        pos: done,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(&mut ws, &ctx, input(0.3, vec![Event::PointerMoved(done)]));
    pass(&mut ws, &ctx, input(0.4, vec![click(true)]));
    pass(&mut ws, &ctx, input(0.5, vec![click(false)]));
    run(&mut ws, &ctx, 0.6);
    assert!(!ws.windows.preferences, "Done closes it");
}
