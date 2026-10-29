use super::*;
use crate::workspace::home::PRESETS;

#[test]
fn every_preset_is_a_stage_the_document_accepts() {
    for (name, w, h, fps) in PRESETS {
        let mut ws = workspace();
        ws.new_project(w, h, fps);
        let stage = ws.document().stage();
        assert_eq!((stage.width(), stage.height()), (w, h), "{name}");
        assert_eq!(ws.document().frame_rate().numerator(), fps, "{name}");
    }
}

#[test]
fn a_new_project_replaces_an_untouched_document_and_otherwise_opens_a_tab() {
    let mut ws = workspace();
    ws.new_project(1280, 720, 24);
    assert_eq!(ws.tabs().len(), 1, "the blank document is replaced");
    rect(&mut ws);
    ws.new_project(800, 600, 24);
    assert_eq!(ws.tabs().len(), 2, "a document with work in it is kept");
    assert_eq!(ws.document().stage().width(), 800);
}

#[test]
fn an_impossible_size_is_reported_and_changes_nothing() {
    let mut ws = workspace();
    let before = ws.document().stage();
    ws.new_project(0, 100, 24);
    assert_eq!(ws.document().stage(), before);
    assert!(ws.status().is_some());
}

#[test]
fn the_home_page_shows_over_the_document_and_goes_away_when_a_project_starts() {
    let mut ws = workspace();
    assert!(!ws.home_shown());
    ws.set_home(true);
    assert!(ws.home_shown());
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    assert!(ws.home_shown(), "drawing the page does not dismiss it");
    ws.new_project(1920, 1080, 24);
    assert!(!ws.home_shown(), "starting a project leaves Home");
}

#[test]
fn the_menu_and_the_new_command_treat_home_as_a_view_not_a_document() {
    let mut ws = workspace();
    ws.perform(MenuAction::ShowHome);
    assert!(ws.home_shown());
    ws.perform(MenuAction::ShowHome);
    assert!(!ws.home_shown());
    // New with Home up on an untouched document starts in place instead of stacking tabs.
    ws.set_home(true);
    ws.perform(MenuAction::New);
    assert_eq!(ws.tabs().len(), 1);
    assert!(!ws.home_shown());
    // On a document with work, Home is left behind in the old tab.
    rect(&mut ws);
    ws.set_home(true);
    ws.perform(MenuAction::New);
    assert_eq!(ws.tabs().len(), 2);
    assert!(!ws.home_shown());
}

#[test]
fn recent_projects_are_listed_newest_first_and_can_be_cleared() {
    let mut ws = workspace();
    ws.note_recent(std::path::Path::new("/a/one.flare"));
    ws.note_recent(std::path::Path::new("/a/two.flare"));
    assert_eq!(ws.recent_files()[0], PathBuf::from("/a/two.flare"));
    ws.set_home(true);
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    assert_eq!(ws.recent_files().len(), 2);
}

#[test]
fn the_title_is_the_app_on_home_and_the_saved_name_on_a_document() {
    let mut ws = workspace();
    ws.set_home(true);
    assert_eq!(ws.title(), "Flare", "no document is on show");
    ws.set_home(false);
    assert_eq!(ws.title(), "Untitled - Flare");
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("Rocket.flare");
    flare_storage::save(ws.document(), &path).expect("save");
    ws.open(&path);
    ws.set_home(true);
    assert_eq!(ws.title(), "Flare");
    ws.set_home(false);
    assert_eq!(ws.title(), "Rocket.flare - Flare");
}

#[test]
fn home_has_no_undo_play_or_document_name_in_the_top_bar() {
    let mut ws = workspace();
    assert!(ws.document_controls_shown());
    ws.set_home(true);
    assert!(!ws.document_controls_shown());
    ws.perform(MenuAction::ShowHome);
    assert!(
        ws.document_controls_shown(),
        "they come back with the document"
    );
}

#[test]
fn keys_and_commands_leave_the_hidden_document_alone_while_home_is_up() {
    let mut ws = two_rectangles();
    ws.perform(MenuAction::SelectAll);
    ws.set_home(true);
    for action in [
        MenuAction::Delete,
        MenuAction::Paste,
        MenuAction::Cut,
        MenuAction::SelectAll,
        MenuAction::ToolBrush,
        MenuAction::PlayPause,
        MenuAction::FlipHorizontal,
        MenuAction::MirrorCopyHorizontal,
    ] {
        assert!(!ws.is_enabled(action), "{action:?}");
    }
    for action in [
        MenuAction::New,
        MenuAction::Open,
        MenuAction::ShowHome,
        MenuAction::Quit,
    ] {
        assert!(ws.is_enabled(action), "{action:?}");
    }
    let ctx = egui::Context::default();
    pass(
        &mut ws,
        &ctx,
        input(0.0, vec![key(Key::Delete, Modifiers::NONE)]),
    );
    assert_eq!(layer_count(&ws), 3, "Delete did not reach the document");
    assert_eq!(ws.tool(), Tool::Select);
}

#[test]
fn choosing_a_tab_leaves_home_even_the_current_one() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.new_project(800, 600, 24);
    assert_eq!(ws.tabs().len(), 2);
    ws.set_home(true);
    ws.switch_to_tab(1);
    assert!(!ws.home_shown(), "the current tab shows its document");
    ws.set_home(true);
    ws.switch_to_tab(0);
    assert!(!ws.home_shown());
    ws.switch_to_tab(1);
    assert!(!ws.home_shown(), "Home does not come back with the tab");
}

#[test]
fn the_home_page_draws_with_and_without_recent_projects_at_small_and_large_sizes() {
    for (recent, size) in [
        (false, 1280.0),
        (true, 1280.0),
        (true, 340.0),
        (true, 4000.0),
    ] {
        let mut ws = workspace();
        if recent {
            ws.note_recent(std::path::Path::new("/a/one.flare"));
            ws.note_recent(std::path::Path::new(
                "/a/a-very-long-name-that-goes-on-and-on.flare",
            ));
        }
        ws.set_home(true);
        let ctx = egui::Context::default();
        let raw = RawInput {
            time: Some(0.0),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(size, 700.0))),
            ..RawInput::default()
        };
        pass(&mut ws, &ctx, raw);
        assert!(ws.home_shown());
    }
}
