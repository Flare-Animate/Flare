//! The keyboard shortcut editor: rebinding, refusals, tool letters from the map, and saving.

use super::*;
use crate::workspace::shortcut_editor::chord_text;

fn shortcut(ws: &Workspace, action: MenuAction) -> Option<String> {
    ws.shortcuts().get(action).map(|a| a.to_string())
}

#[test]
fn the_default_set_shows_the_builtin_keys() {
    let ws = workspace();
    assert_eq!(ws.shortcuts().set(), "flare");
    assert_eq!(
        ws.shortcut_label(MenuAction::Preferences),
        MenuAction::Preferences.shortcut_text()
    );
}

#[test]
fn keys_another_command_holds_are_taken_from_it_and_bad_keys_change_nothing() {
    let mut ws = workspace();
    let before = ws.shortcuts().clone();
    assert!(ws.set_shortcut(MenuAction::Redo, "not keys").is_err());
    assert_eq!(ws.shortcuts(), &before);
    let taken = ws
        .set_shortcut(MenuAction::Redo, "CmdOrCtrl+Z")
        .expect("Undo's keys are taken");
    assert_eq!(taken, Some(MenuAction::Undo));
    assert_eq!(
        shortcut(&ws, MenuAction::Redo).as_deref(),
        Some("CmdOrCtrl+Z")
    );
    assert_eq!(shortcut(&ws, MenuAction::Undo), None);
    let notice = ws
        .shortcut_editor
        .notice
        .clone()
        .expect("the editor says so");
    assert!(notice.contains(&ws.label(MenuAction::Undo)), "{notice}");
    // Reset gives Undo its keys back, from Redo.
    assert_eq!(
        ws.reset_shortcut(MenuAction::Undo).expect("reset"),
        Some(MenuAction::Redo)
    );
    assert_eq!(
        shortcut(&ws, MenuAction::Undo).as_deref(),
        Some("CmdOrCtrl+Z")
    );
    assert_eq!(shortcut(&ws, MenuAction::Redo), None);
    assert!(ws.shortcuts().conflicts().is_empty());
    // Free keys take nothing and say nothing.
    assert_eq!(
        ws.set_shortcut(MenuAction::Redo, "CmdOrCtrl+Alt+K"),
        Ok(None)
    );
    assert!(ws.shortcut_editor.notice.is_none());
}

#[test]
fn a_taken_tool_letter_picks_the_new_tool() {
    let mut ws = workspace();
    // B is the Brush's: given to the Pen, it picks the Pen and the Brush has no letter.
    assert_eq!(
        ws.set_shortcut(MenuAction::ToolPen, "B"),
        Ok(Some(MenuAction::ToolBrush))
    );
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    ws.set_tool(Tool::Select);
    let mut out = ctx.run_ui(input(0.1, vec![key(Key::B, Modifiers::NONE)]), |ui| {
        ws.show(ui)
    });
    out.textures_delta.clear();
    assert_eq!(ws.tool(), Tool::Pen);
}

#[test]
fn a_rebound_tool_letter_picks_the_tool_and_the_old_letter_does_nothing() {
    let mut ws = workspace();
    ws.set_shortcut(MenuAction::ToolBrush, "J").expect("free");
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    ws.set_tool(Tool::Select);
    let mut out = ctx.run_ui(input(0.1, vec![key(Key::J, Modifiers::NONE)]), |ui| {
        ws.show(ui)
    });
    out.textures_delta.clear();
    assert_eq!(ws.tool(), Tool::Brush);
    ws.set_tool(Tool::Select);
    let mut out = ctx.run_ui(input(0.2, vec![key(Key::B, Modifiers::NONE)]), |ui| {
        ws.show(ui)
    });
    out.textures_delta.clear();
    assert_eq!(ws.tool(), Tool::Select, "B was given up");
}

#[test]
fn another_set_changes_the_keys_and_drops_edits() {
    let mut ws = workspace();
    ws.set_shortcut(MenuAction::Redo, "CmdOrCtrl+Alt+K")
        .expect("free");
    ws.choose_shortcut_set("opentoonz").expect("set");
    assert_eq!(shortcut(&ws, MenuAction::ToolSelect).as_deref(), Some("S"));
    assert_eq!(
        shortcut(&ws, MenuAction::Redo).as_deref(),
        Some("CmdOrCtrl+Y")
    );
    assert!(ws.choose_shortcut_set("nope").is_err());
    assert_eq!(ws.shortcuts().set(), "opentoonz");
}

#[test]
fn the_window_captures_the_next_key_chord_and_escape_cancels() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    ws.windows.shortcuts = true;
    ws.shortcut_editor.capturing = Some(MenuAction::Redo);
    let chord = Modifiers {
        command: true,
        alt: true,
        ..Modifiers::NONE
    };
    let mut out = ctx.run_ui(input(0.1, vec![key(Key::K, chord)]), |ui| ws.show(ui));
    out.textures_delta.clear();
    assert_eq!(
        shortcut(&ws, MenuAction::Redo).as_deref(),
        Some("CmdOrCtrl+Alt+K")
    );
    assert!(ws.shortcut_editor.capturing.is_none());
    ws.shortcut_editor.capturing = Some(MenuAction::Redo);
    let mut out = ctx.run_ui(input(0.2, vec![key(Key::Escape, Modifiers::NONE)]), |ui| {
        ws.show(ui)
    });
    out.textures_delta.clear();
    assert_eq!(
        shortcut(&ws, MenuAction::Redo).as_deref(),
        Some("CmdOrCtrl+Alt+K")
    );
    assert!(ws.shortcut_editor.capturing.is_none());
}

#[test]
fn chords_are_written_in_the_registry_spelling() {
    assert_eq!(chord_text(Key::F5, Modifiers::SHIFT), "Shift+F5");
    assert_eq!(chord_text(Key::Comma, Modifiers::COMMAND), "CmdOrCtrl+,");
    assert_eq!(chord_text(Key::ArrowUp, Modifiers::NONE), "Up");
}

#[test]
fn edited_shortcuts_are_kept_with_the_preferences_and_a_corrupt_file_falls_back() {
    let mut ws = workspace();
    assert!(
        ws.preferences().shortcuts.is_empty(),
        "the default is not stored"
    );
    ws.choose_shortcut_set("tahoma").expect("set");
    ws.set_shortcut(MenuAction::Redo, "CmdOrCtrl+Alt+K")
        .expect("free");
    let saved = ws.preferences();
    let mut fresh = workspace();
    fresh.apply_preferences(&saved);
    assert_eq!(fresh.shortcuts(), ws.shortcuts());
    let corrupt = Preferences {
        shortcuts: "{\"schema\":1,\"set\":\"flare\",\"bindings\":{\"Close\":\"CmdOrCtrl+O\"}}"
            .into(),
        ..Preferences::default()
    };
    fresh.apply_preferences(&corrupt);
    assert_eq!(fresh.shortcuts(), &flare_menubar::ShortcutMap::default());
}

/// Every piece of text the Keyboard Shortcuts window paints in one pass.
fn editor_text(ws: &mut Workspace, time: f64) -> Vec<String> {
    ws.windows.shortcuts = true;
    let ctx = egui::Context::default();
    // A window is measured on its first pass and drawn on the next.
    run(ws, &ctx, time);
    let mut out = ctx.run_ui(input(time + 0.01, vec![]), |ui| ws.show(ui));
    out.textures_delta.clear();
    out.shapes
        .iter()
        .filter_map(|s| match &s.shape {
            egui::epaint::Shape::Text(t) => Some(t.galley.job.text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_search_narrows_the_list_by_name_in_the_persons_language_or_by_keys() {
    let mut ws = workspace();
    ws.set_system_locale("de-DE");
    let found = ws.search_shortcuts("speichern unter");
    assert_eq!(found, [MenuAction::SaveAs]);
    // Keys are found the same way in every language.
    assert_eq!(ws.search_shortcuts("cmd+o"), [MenuAction::Open]);
    // The window lists only what the search finds.
    ws.set_shortcut_search("speichern unter");
    let text = editor_text(&mut ws, 0.0);
    assert!(
        text.iter().any(|t| t.starts_with("Speichern unter")),
        "{text:?}"
    );
    assert!(!text.iter().any(|t| t.starts_with("Öffnen")), "{text:?}");
    // Nothing found says so, with what was typed.
    ws.set_shortcut_search("zzzz");
    let text = editor_text(&mut ws, 0.1);
    assert!(text.iter().any(|t| t.contains("zzzz")), "{text:?}");
    // An empty search lists everything again, a long one is cut.
    ws.set_shortcut_search(&"x".repeat(10_000));
    assert_eq!(
        ws.shortcut_editor.query.chars().count(),
        flare_menubar::MAX_SEARCH_CHARS
    );
    ws.set_shortcut_search("");
    assert!(ws.search_shortcuts("").len() > 40);
}
