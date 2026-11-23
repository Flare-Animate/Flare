//! The interface size: larger than the design size to start with, chosen in Preferences, kept
//! with them, and applied to the whole window as egui's zoom.

use super::*;

fn texts(ws: &mut Workspace, ctx: &egui::Context, time: f64) -> Vec<String> {
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_interface_starts_larger_than_its_design_size() {
    let mut ws = Workspace::new(document(), None);
    assert_eq!(ws.preferences().interface_size, flare_ui::scale::DEFAULT);
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    assert!((ctx.zoom_factor() - flare_ui::scale::DEFAULT).abs() < 1e-4);
    assert!(ctx.zoom_factor() > 1.0);
}

#[test]
fn a_chosen_size_is_applied_kept_and_brought_into_range() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut prefs = ws.preferences();
    prefs.interface_size = 1.5;
    ws.apply_preferences(&prefs);
    // The test window is 1280 by 900 points at whatever zoom is in force, so it is 1280 by 900 on
    // the screen at first, which leaves room for 4/3 and no more; at that it is larger, and 1.5 fits.
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    assert!((ctx.zoom_factor() - 1280.0 / 960.0).abs() < 1e-3);
    // A new zoom takes hold at the pass after the one that sets it.
    run(&mut ws, &ctx, 0.15);
    run(&mut ws, &ctx, 0.17);
    assert!((ctx.zoom_factor() - 1.5).abs() < 1e-4);
    let mut fresh = workspace();
    fresh
        .restore_preferences(&ws.preferences_json().expect("json"))
        .expect("restored");
    assert_eq!(fresh.preferences().interface_size, 1.5);
    fresh
        .restore_preferences(r#"{"interface_size": 40.0}"#)
        .expect("odd");
    assert_eq!(fresh.preferences().interface_size, flare_ui::scale::MAX);
    // A file from before the setting starts at the default; Restore defaults goes back to it.
    fresh.restore_preferences("{}").expect("old file");
    assert_eq!(fresh.preferences().interface_size, flare_ui::scale::DEFAULT);
    ws.apply_preferences(&Preferences::default());
    run(&mut ws, &ctx, 0.2);
    run(&mut ws, &ctx, 0.3);
    assert!((ctx.zoom_factor() - flare_ui::scale::DEFAULT).abs() < 1e-4);
}

#[test]
fn the_size_row_is_in_general_and_in_the_chosen_language() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    ws.windows.preferences = true;
    let mut all = Vec::new();
    for step in 0..5 {
        all = texts(&mut ws, &ctx, f64::from(step) * 0.1);
    }
    for wanted in ["Interface size", "100%"] {
        assert!(all.iter().any(|t| t == wanted), "{wanted}: {all:?}");
    }
    let mut prefs = ws.preferences();
    prefs.language = "de".to_owned();
    ws.apply_preferences(&prefs);
    for step in 5..10 {
        all = texts(&mut ws, &ctx, f64::from(step) * 0.1);
    }
    assert!(all.iter().any(|t| t == "Größe der Oberfläche"), "{all:?}");
}
