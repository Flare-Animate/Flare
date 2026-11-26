//! The stage's rulers: their numbers stay inside the ruler strips.

use super::*;

/// The numbers the rulers draw: their text, where they show and what they are clipped to.
fn ruler_numbers(panel: Rect, image: Rect, scale: f32) -> Vec<(String, Rect, Rect)> {
    let (ctx, ws) = (egui::Context::default(), workspace());
    let mut out = ctx.run_ui(input(0.0, vec![]), |ui| {
        let painter = ui.painter_at(panel);
        ws.stage_rulers(&painter, panel, image, scale);
    });
    out.textures_delta.clear();
    out.shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some((
                t.galley.text().to_owned(),
                t.visual_bounding_rect(),
                s.clip_rect,
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn the_left_rulers_numbers_fit_its_width_and_stop_at_the_panel() {
    let panel = Rect::from_min_size(Pos2::new(40.0, 30.0), Vec2::new(900.0, 700.0));
    let image = Rect::from_min_size(Pos2::new(140.0, 130.0), Vec2::new(1152.0, 648.0));
    let numbers = ruler_numbers(panel, image, 0.6);
    let left: Vec<_> = numbers
        .iter()
        .filter(|(_, at, _)| at.center().x < panel.min.x + 16.0)
        .collect();
    assert!(
        left.iter().any(|(text, _, _)| text.len() >= 4),
        "four-digit numbers are drawn on the left: {numbers:?}"
    );
    for (text, at, clip) in &left {
        assert!(
            at.min.x >= panel.min.x - 0.5 && at.max.x <= panel.min.x + 16.5,
            "{text} at {at:?} spills out of the left ruler"
        );
        assert!(
            clip.max.x <= panel.min.x + 16.0 && clip.max.y <= panel.max.y,
            "{text} is not clipped to its ruler: {clip:?}"
        );
    }
    for (text, _, clip) in &numbers {
        assert!(
            clip.min.x >= panel.min.x && clip.max.x <= panel.max.x && clip.max.y <= panel.max.y,
            "{text} may draw outside the stage panel: {clip:?}"
        );
    }
}
