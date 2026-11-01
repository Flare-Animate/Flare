//! The status bar at the bottom and the zoom box above the stage.

use super::*;

#[test]
fn the_stage_draws_no_zoom_percentage_of_its_own_the_stage_bar_has_it() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    ws.set_stage_percent(200.0);
    let raw = RawInput {
        time: Some(1.0),
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1200.0, 800.0))),
        ..RawInput::default()
    };
    let mut out = ctx.run_ui(raw, |ui| ws.show(ui));
    out.textures_delta.clear();
    // The stage bar has the number in a box (14 pt); the stage itself drew a second, loose 12 pt
    // one in its corner.
    let loose = out.shapes.iter().any(|s| {
        matches!(&s.shape, egui::epaint::Shape::Text(t)
            if t.galley.text().ends_with('%') && t.galley.job.sections[0].format.font_id.size == 12.0)
    });
    assert!(!loose, "no second zoom label in the stage's corner");
    // The stage bar's box draws the number and its suffix as two texts; nothing else, the status
    // bar included, draws the zoom as a label of its own.
    let drew = |wanted: &str| {
        out.shapes
            .iter()
            .any(|s| matches!(&s.shape, egui::epaint::Shape::Text(t) if t.galley.text() == wanted))
    };
    assert!(
        drew("200") && drew("%"),
        "the stage bar still shows the zoom"
    );
    assert!(!drew("200%"), "no loose zoom label anywhere else");
}

#[test]
fn the_status_bar_shows_the_stage_size_and_frame_rate_and_a_click_opens_document_settings() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let stage = ws.document().stage();
    let wanted = format!("{} × {} px · 24 fps", stage.width(), stage.height());
    let mut at = None;
    for step in 0..4 {
        let raw = input(f64::from(step) * 0.1, vec![]);
        let mut out = ctx.run_ui(raw, |ui| ws.show(ui));
        out.textures_delta.clear();
        at = out.shapes.iter().rev().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == wanted => {
                Some(t.visual_bounding_rect().center())
            }
            _ => None,
        });
    }
    let at = at.expect("the document's size and frame rate are in the status bar");
    // A drawing tool in hand: the panel is about the tool until something asks for the document.
    ws.set_tool(Tool::Rectangle);
    assert_eq!(ws.props_context(), PropsContext::Tool);
    for (time, event) in [
        (1.0, Event::PointerMoved(at)),
        (
            1.1,
            Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ),
        (
            1.2,
            Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ),
    ] {
        pass(&mut ws, &ctx, input(time, vec![event]));
    }
    assert_eq!(
        ws.props_context(),
        PropsContext::Document,
        "a click opens the document settings"
    );
}
