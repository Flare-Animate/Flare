use super::*;
use crate::workspace::types::ColorTarget;

#[test]
fn a_swatch_sets_the_color_being_set_and_the_persons_own_are_kept_with_the_preferences() {
    let mut ws = workspace();
    let blue = Rgba8::opaque(0, 51, 255);
    ws.choose_swatch(blue);
    assert_eq!(ws.paint.fill, Color32::from_rgb(0, 51, 255));
    ws.color_target = ColorTarget::Stroke;
    ws.choose_swatch(Rgba8::opaque(255, 0, 0));
    assert_eq!(ws.paint.stroke_color, Color32::from_rgb(255, 0, 0));
    assert_eq!(
        ws.paint.fill,
        Color32::from_rgb(0, 51, 255),
        "the fill is untouched"
    );

    assert!(ws.add_swatch(), "the stroke color is kept");
    assert!(!ws.add_swatch(), "once");
    ws.color_target = ColorTarget::Fill;
    assert!(ws.add_swatch());
    assert_eq!(
        ws.paint.swatches.colors(),
        &[Rgba8::opaque(255, 0, 0), blue]
    );
    let preferences = ws.preferences();
    assert_eq!(
        preferences.swatches,
        vec![[255, 0, 0, 255], [0, 51, 255, 255]]
    );
    let mut next = workspace();
    next.apply_preferences(&preferences);
    assert_eq!(next.paint.swatches, ws.paint.swatches);
    ws.remove_swatch(0);
    ws.remove_swatch(9);
    assert_eq!(ws.paint.swatches.colors(), &[blue]);
}

#[test]
fn a_full_list_says_so() {
    let mut ws = workspace();
    for i in 0..flare_core::swatches::MAX_CUSTOM_SWATCHES {
        let i = u8::try_from(i).expect("small");
        ws.paint.fill = Color32::from_rgb(i, 0, 0);
        assert!(ws.add_swatch());
    }
    ws.paint.fill = Color32::from_rgb(0, 0, 9);
    assert!(!ws.add_swatch());
    assert!(
        ws.status().is_some_and(|s| s.contains("120")),
        "{:?}",
        ws.status()
    );
}

#[test]
fn the_panel_is_in_the_strip_and_a_click_on_a_cell_picks_its_color() {
    let mut ws = workspace();
    assert!(ws.panels.layout.in_strip(Pane::Swatches));
    ws.perform(MenuAction::ShowSwatches);
    assert!(ws.pane_is_front(Pane::Swatches));
    let ctx = egui::Context::default();
    let mut panel = Rect::NOTHING;
    for step in 0..3 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| {
            panel = ui.max_rect();
            ws.swatches_panel(ui);
        });
        out.textures_delta.clear();
    }
    // The first cell of the greys, at the top left under the label, is black.
    ws.paint.fill = Color32::WHITE;
    let at = panel.min + egui::vec2(4.0, 26.0);
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    for (time, events) in [
        (0.5, vec![Event::PointerMoved(at)]),
        (0.6, vec![click(true)]),
        (0.7, vec![click(false)]),
    ] {
        let mut out = ctx.run_ui(input(time, events), |ui| ws.swatches_panel(ui));
        out.textures_delta.clear();
    }
    assert_eq!(ws.paint.fill, Color32::BLACK);
}
