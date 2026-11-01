//! The modified time, size and search on the Home page's recent projects.

use super::*;
use std::time::Duration;

/// All the text a pass drew.
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
fn the_list_shows_when_the_file_changed_and_how_big_it_is_each_under_its_heading() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("Real.flare");
    std::fs::write(&path, vec![0u8; 3000]).expect("write");
    let mut ws = workspace();
    ws.note_recent(&path);
    ws.note_recent(&dir.path().join("Missing.flare"));
    ws.set_home(true);
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    let recent = ws.recent_files().to_vec();
    assert!(
        ws.recent_info.ask(&recent, || {}).is_none(),
        "the page already asked"
    );
    for _ in 0..200 {
        if ws.recent_info.get(&path).is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    let drawn = texts(&mut ws, &ctx, 0.1);
    for wanted in [
        "just now",
        "2.9 KB",
        "Date modified",
        "Size",
        "Kind",
        "Flare project",
    ] {
        assert!(drawn.iter().any(|t| t == wanted), "{wanted}: {drawn:?}");
    }
    assert!(drawn.iter().any(|t| t == "Not found"), "{drawn:?}");
}

#[test]
fn the_search_narrows_the_cards_and_says_when_nothing_matches() {
    let mut ws = workspace();
    ws.note_recent(std::path::Path::new("/a/walk-cycle.flare"));
    ws.note_recent(std::path::Path::new("/a/logo.flare"));
    ws.set_home(true);
    let ctx = egui::Context::default();
    let all = texts(&mut ws, &ctx, 0.0);
    assert!(all.iter().any(|t| t == "walk-cycle.flare") && all.iter().any(|t| t == "logo.flare"));
    ws.home_search = "WALK".to_owned();
    let some = texts(&mut ws, &ctx, 0.1);
    assert!(some.iter().any(|t| t == "walk-cycle.flare"));
    assert!(!some.iter().any(|t| t == "logo.flare"));
    ws.home_search = "zzz".to_owned();
    let none = texts(&mut ws, &ctx, 0.2);
    assert!(
        none.iter().any(|t| t.contains("No recent project matches")),
        "{none:?}"
    );
    assert_eq!(ws.recent_files().len(), 2, "searching forgets nothing");
}

/// Where the text `wanted` was drawn in a pass.
fn text_at(ws: &mut Workspace, ctx: &egui::Context, time: f64, wanted: &str) -> Option<Pos2> {
    let mut out = ctx.run_ui(input(time, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes.iter().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == wanted => {
            Some(t.visual_bounding_rect().center())
        }
        _ => None,
    })
}

#[test]
fn clearing_the_recent_list_is_in_preferences_and_not_on_home() {
    let mut ws = workspace();
    ws.note_recent(std::path::Path::new("/a/one.flare"));
    ws.set_home(true);
    let ctx = egui::Context::default();
    let home = texts(&mut ws, &ctx, 0.0);
    assert!(
        !home.iter().any(|t| t.contains("Clear")),
        "nothing on Home forgets the list: {home:?}"
    );
    ws.windows.preferences = true;
    // A window is laid out on its first pass and shown from the second.
    run(&mut ws, &ctx, 0.05);
    let at = text_at(&mut ws, &ctx, 0.1, "Clear recent projects").expect("the button is drawn");
    let click = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(&mut ws, &ctx, input(0.2, vec![Event::PointerMoved(at)]));
    pass(&mut ws, &ctx, input(0.3, vec![click(true)]));
    pass(&mut ws, &ctx, input(0.4, vec![click(false)]));
    assert!(ws.recent_files().is_empty(), "the button forgot the list");
}

#[test]
fn a_card_says_reading_until_the_files_have_been_looked_at() {
    let mut ws = workspace();
    let path = std::path::Path::new("/a/one.flare");
    ws.note_recent(path);
    let recent = ws.recent_files().to_vec();
    // Showing Home forgets what was read, so it comes first; then nothing is looked at.
    ws.set_home(true);
    ws.recent_info.mark_asked(&recent);
    let ctx = egui::Context::default();
    let drawn = texts(&mut ws, &ctx, 0.0);
    assert!(drawn.iter().any(|t| t == "Reading…"), "{drawn:?}");
}
