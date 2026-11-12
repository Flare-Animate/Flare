//! File > New asks for the kind and size of the project in Animate's New Document dialog.

use super::layer_names::{click, find, settle};
use super::*;
use egui::PointerButton;

#[test]
fn file_new_opens_the_dialog_and_a_kind_a_size_and_create_make_the_project_in_a_new_tab() {
    let mut ws = workspace();
    rect(&mut ws); // work in the open document, which must be kept
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    ws.perform(MenuAction::New);
    assert!(ws.new_document_shown(), "File > New asks first");
    assert_eq!((ws.parked.len() + 1), 1, "and makes nothing yet");
    // A new window measures itself, unseen, on its first pass.
    settle(&mut ws, &ctx, &mut time);
    for (label, wanted) in [("Ads", "Leaderboard"), ("Leaderboard", "Create")] {
        time += 0.1;
        let at = find(&mut ws, &ctx, time, vec![], label).expect(label);
        click(&mut ws, &ctx, &mut time, at, PointerButton::Primary);
        time += 0.1;
        assert!(
            find(&mut ws, &ctx, time, vec![], wanted).is_some(),
            "{wanted} after {label}"
        );
    }
    assert_eq!(
        ws.home.new_document.map(|d| (d.width, d.height, d.fps)),
        Some((728, 90, 24)),
        "the Leaderboard's numbers"
    );
    time += 0.1;
    let create = find(&mut ws, &ctx, time, vec![], "Create").expect("Create");
    click(&mut ws, &ctx, &mut time, create, PointerButton::Primary);
    assert!(!ws.new_document_shown());
    assert_eq!(
        (ws.parked.len() + 1),
        2,
        "a tab of its own, the drawing kept in the other"
    );
    let stage = ws.document().stage();
    assert_eq!((stage.width(), stage.height()), (728, 90));
}

#[test]
fn escape_closes_the_dialog_and_makes_nothing() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    ws.perform(MenuAction::New);
    settle(&mut ws, &ctx, &mut time); // the dialog is up before the key
    assert!(ws.new_document_shown());
    time += 0.1;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    assert!(!ws.new_document_shown());
    assert_eq!((ws.parked.len() + 1), 1);
}

#[test]
fn the_details_are_a_column_and_the_dialog_is_no_wider_than_its_parts() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    ws.perform(MenuAction::New);
    settle(&mut ws, &ctx, &mut time);
    let mut out = ctx.run_ui(input(time + 0.1, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    let at = |wanted: &str| {
        out.shapes
            .iter()
            .rev()
            .find_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) if t.galley.text() == wanted => {
                    Some(t.visual_bounding_rect())
                }
                _ => None,
            })
            .unwrap_or_else(|| panic!("{wanted} is not drawn"))
    };
    let (units, rate, background) = (at("Units"), at("Frame rate"), at("Background color"));
    // Once laid out in a row, these crowded each other on one line.
    assert!(units.bottom() < rate.top() && rate.bottom() < background.top());
    assert!((units.left() - rate.left()).abs() < 2.0, "one column");
    let (first_tab, last_tab) = (at("Character Animation"), at("Advanced"));
    assert!(
        last_tab.right() - first_tab.left() < 760.0,
        "the tabs fit the dialog, not the window"
    );
    assert!(
        at("Create").right() < 1280.0 - 100.0,
        "the dialog sits inside the window with room to spare"
    );
}
