//! The menu on a right click of the Play button at the top.

use super::layer_names::press;
use super::*;
use egui::PointerButton;

#[test]
fn the_play_buttons_menu_offers_test_movie_and_choosing_it_asks_for_the_player() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let mut at = None;
    for step in 0..4 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| {
            ws.transport_menu(ui)
        });
        out.textures_delta.clear();
        at = out.shapes.iter().rev().find_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) if t.galley.text() == "Test Movie" => {
                Some(t.visual_bounding_rect().center())
            }
            _ => None,
        });
    }
    let at = at.expect("the menu offers Test Movie");
    assert!(!ws.take_test_movie_request(), "not asked for yet");
    for (time, events) in [
        (1.0, vec![Event::PointerMoved(at)]),
        (1.1, vec![press(at, PointerButton::Primary, true)]),
        (1.2, vec![press(at, PointerButton::Primary, false)]),
        (1.3, vec![]),
    ] {
        let mut out = ctx.run_ui(input(time, events), |ui| ws.transport_menu(ui));
        out.textures_delta.clear();
    }
    assert!(
        ws.take_test_movie_request(),
        "choosing it asks for the player"
    );
}
