//! The grid over the stage: it shows on any stage, and its spacing and color can be set.

use super::*;

/// Every straight line drawn in a few passes of the whole window.
fn drawn_lines(ws: &mut Workspace) -> Vec<(egui::Pos2, egui::Pos2, egui::Color32)> {
    let ctx = egui::Context::default();
    let mut lines = Vec::new();
    for step in 0..3 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| ws.show(ui));
        out.textures_delta.clear();
        lines = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::LineSegment { points, stroke } => {
                    Some((points[0], points[1], stroke.color))
                }
                _ => None,
            })
            .collect();
    }
    lines
}

/// How far `ink` laid over `background` is from the background, in the brightest channel.
fn contrast(ink: egui::Color32, background: flare_core::Rgba8) -> u8 {
    let [r, g, b, _] = background.to_array();
    let keep = 1.0 - f32::from(ink.a()) / 255.0;
    [(ink.r(), r), (ink.g(), g), (ink.b(), b)]
        .map(|(i, c)| ((f32::from(i) + f32::from(c) * keep) - f32::from(c)).abs() as u8)
        .into_iter()
        .max()
        .unwrap_or(0)
}

#[test]
fn the_grid_shows_on_a_white_stage_and_on_a_dark_one() {
    for background in [
        flare_core::Rgba8::opaque(255, 255, 255),
        flare_core::Rgba8::opaque(20, 20, 24),
    ] {
        let mut ws = workspace();
        ws.set_stage_background(background);
        let without = drawn_lines(&mut ws);
        ws.perform(MenuAction::ToggleGrid);
        let with = drawn_lines(&mut ws);
        // The stage is empty, so what is new inside its rectangle is the grid.
        let image = ws
            .stage_view
            .image_rect(ws.stage_panel, ws.stage_extent())
            .expand(0.5);
        let grid: Vec<_> = with
            .iter()
            .filter(|l| !without.contains(l))
            .filter(|(a, b, _)| image.contains(*a) && image.contains(*b))
            .collect();
        assert!(grid.len() > 20, "{background:?}: {} grid lines", grid.len());
        for (_, _, ink) in grid {
            let seen = contrast(*ink, background);
            assert!(
                seen >= 60,
                "{background:?}: a grid line of {ink:?} is only {seen} from the stage"
            );
        }
    }
}

/// The grid's lines inside the stage, as (x of each line down, y of each line across), in the
/// order drawn, for lines of `ink`.
fn grid_lines(ws: &mut Workspace, ink: egui::Color32) -> (Vec<f32>, Vec<f32>) {
    let lines = drawn_lines(ws);
    let image = ws
        .stage_view
        .image_rect(ws.stage_panel, ws.stage_extent())
        .expand(0.5);
    let ours = lines
        .iter()
        .filter(|(a, b, c)| *c == ink && image.contains(*a) && image.contains(*b));
    let down = ours.clone().filter(|(a, b, _)| a.x == b.x).map(|l| l.0.x);
    let across = ours.filter(|(a, b, _)| a.y == b.y).map(|l| l.0.y);
    (down.collect(), across.collect())
}

/// Every gap between neighbouring lines, on the screen.
fn gaps(at: &[f32]) -> Vec<f32> {
    at.windows(2).map(|w| w[1] - w[0]).collect()
}

#[test]
fn the_grid_takes_a_chosen_color_and_its_own_spacing_across_and_down() {
    let red = egui::Color32::from_rgb(200, 0, 0);
    let mut ws = workspace();
    ws.perform(MenuAction::ToggleGrid);
    ws.set_grid_spacing(40.0, 20.0);
    ws.set_grid_color(Some(red));
    let (down, across) = grid_lines(&mut ws, red);
    assert!(down.len() > 3 && across.len() > 3, "{down:?} {across:?}");
    let scale = ws.stage_view.scale(ws.stage_panel, ws.stage_extent());
    for gap in gaps(&down) {
        assert!((gap - 40.0 * scale).abs() < 0.01, "{gap} across");
    }
    for gap in gaps(&across) {
        assert!((gap - 20.0 * scale).abs() < 0.01, "{gap} down");
    }
    // Automatic again: the chosen color is gone and the stage's own ink is back.
    ws.set_grid_color(None);
    assert_eq!(grid_lines(&mut ws, red), (vec![], vec![]));
    let ink = ws.grid_ink();
    assert!(!grid_lines(&mut ws, ink).0.is_empty());
}

#[test]
fn the_spacing_is_kept_in_range_and_moves_snap_each_way_apart() {
    let mut ws = workspace();
    ws.set_grid_spacing(0.0, 1e9);
    assert_eq!(ws.grid_spacing(), Vec2::new(1.0, 500.0));
    ws.set_grid_spacing(f32::NAN, f32::INFINITY);
    assert_eq!(
        ws.grid_spacing(),
        Vec2::splat(20.0),
        "not a number is the default"
    );
    assert_eq!(
        snap_point(Pos2::new(29.0, 31.0), Vec2::new(10.0, 25.0)),
        Pos2::new(30.0, 25.0)
    );
    rect(&mut ws); // 10..110 x 10..60
    ws.perform(MenuAction::ToggleSnap);
    ws.set_grid_spacing(40.0, 10.0);
    ws.perform(MenuAction::SelectAll);
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.update_drag(Pos2::new(77.0, 49.0)); // moved by (27, 19)
    ws.end_drag();
    let b = ws.selection_bounds().expect("bounds");
    assert_eq!(
        (b.min.x, b.min.y),
        (50.0, 30.0),
        "10 + 40 across, 10 + 20 down"
    );
}

#[test]
fn the_grid_settings_are_kept_with_the_preferences_and_old_files_still_read() {
    let mut ws = workspace();
    let tint = egui::Color32::from_rgba_unmultiplied(10, 20, 30, 128);
    ws.set_grid_spacing(12.0, 30.0);
    ws.set_grid_color(Some(tint));
    let json = ws.preferences_json().expect("json");
    let mut other = workspace();
    other.restore_preferences(&json).expect("restore");
    assert_eq!(other.grid_spacing(), Vec2::new(12.0, 30.0));
    assert_eq!(other.grid_color(), Some(tint));
    // A file from before the grid had two spacings and a color: one spacing for both, automatic.
    other
        .restore_preferences(r#"{"grid_size": 15.0}"#)
        .expect("old file");
    assert_eq!(other.grid_spacing(), Vec2::splat(15.0));
    assert_eq!(other.grid_color(), None);
}

/// Where a button named `name` is, by what a screen reader is told.
fn button_named(ws: &mut Workspace, ctx: &egui::Context, name: &str) -> Option<Pos2> {
    ctx.enable_accesskit();
    let mut found = None;
    for step in 0..3 {
        let mut out = ctx.run_ui(input(10.0 + f64::from(step) * 0.1, vec![]), |ui| {
            ws.show(ui)
        });
        out.textures_delta.clear();
        if let Some(update) = out.platform_output.accesskit_update {
            found = update
                .nodes
                .iter()
                .find(|(_, n)| n.role() == egui::accesskit::Role::Button && n.label() == Some(name))
                .and_then(|(_, n)| n.bounds())
                .map(|r| Pos2::new(((r.x0 + r.x1) / 2.0) as f32, ((r.y0 + r.y1) / 2.0) as f32));
        }
    }
    found
}

fn click(ws: &mut Workspace, ctx: &egui::Context, at: Pos2, time: f64) {
    let press = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(ws, ctx, input(time, vec![Event::PointerMoved(at)]));
    pass(ws, ctx, input(time + 0.05, vec![press(true)]));
    pass(ws, ctx, input(time + 0.1, vec![press(false)]));
}

#[test]
fn the_arrow_by_the_grid_button_opens_its_settings_and_reset_puts_them_back() {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut ws = workspace();
    ws.set_grid_spacing(40.0, 60.0);
    ws.set_grid_color(Some(egui::Color32::RED));
    let arrow = button_named(&mut ws, &ctx, "Grid settings").expect("the arrow is in the bar");
    click(&mut ws, &ctx, arrow, 1.0);
    let reset = button_named(&mut ws, &ctx, "Reset grid").expect("the popup is open");
    for name in ["Horizontal spacing", "Vertical spacing", "Automatic color"] {
        assert!(
            drawn_text(&mut ws, &ctx).iter().any(|t| t == name),
            "{name} is in the popup"
        );
    }
    click(&mut ws, &ctx, reset, 20.0);
    assert_eq!(ws.grid_spacing(), Vec2::splat(20.0));
    assert_eq!(ws.grid_color(), None);
}

fn drawn_text(ws: &mut Workspace, ctx: &egui::Context) -> Vec<String> {
    let mut out = ctx.run_ui(input(15.0, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect()
}
