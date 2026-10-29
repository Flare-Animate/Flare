use super::*;

/// Every panel in one tab group, so bringing one to the front is visible in the layout.
fn all_in_one_stack(ws: &mut Workspace) {
    let mut tiles = egui_tiles::Tiles::default();
    let panes: Vec<_> = Pane::ALL
        .into_iter()
        .map(|pane| tiles.insert_pane(pane))
        .collect();
    let root = tiles.insert_tab_tile(panes);
    let tree = egui_tiles::Tree::new(egui::Id::unique("stack"), root, tiles);
    ws.restore_layout(&layout::to_json(&tree).expect("serialize"))
        .expect("a tab group of every panel is valid");
}

#[test]
fn window_menu_entries_bring_their_panel_to_the_front() {
    let mut ws = workspace();
    all_in_one_stack(&mut ws);
    let json = |ws: &Workspace| ws.layout_json().expect("serialize");
    ws.perform(MenuAction::ShowHistory);
    let history = json(&ws);
    ws.perform(MenuAction::ShowLibrary);
    let library = json(&ws);
    assert_ne!(history, library, "a different panel came to the front");
    ws.perform(MenuAction::ShowHistory);
    assert_eq!(json(&ws), history, "and back again");
}

#[test]
fn every_window_entry_is_enabled_and_named_for_its_panel() {
    let ws = workspace();
    for (action, name) in [
        (MenuAction::ShowProperties, "Properties"),
        (MenuAction::ShowLibrary, "Library"),
        (MenuAction::ShowColor, "Color"),
        (MenuAction::ShowAlign, "Align"),
        (MenuAction::ShowTransform, "Transform"),
        (MenuAction::ShowHistory, "History"),
        (MenuAction::ShowTimeline, "Timeline"),
        (MenuAction::ShowOutput, "Output"),
    ] {
        assert!(ws.is_enabled(action), "{name}");
        assert_eq!(ws.label(action), name);
    }
}

#[test]
fn help_windows_open_and_the_shortcut_list_names_real_commands() {
    let mut ws = workspace();
    ws.perform(MenuAction::About);
    ws.perform(MenuAction::KeyboardShortcuts);
    let lines = ws.shortcut_lines();
    let keys = |name: &str| {
        lines
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, k)| k.clone())
    };
    assert!(keys("Undo").is_some_and(|k| k.contains('Z')));
    assert_eq!(keys("Select / Move").as_deref(), Some("V"));
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0); // both windows draw without panicking
}
