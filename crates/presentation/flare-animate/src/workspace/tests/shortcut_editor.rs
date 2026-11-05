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
fn a_conflict_is_refused_and_changes_nothing() {
    let mut ws = workspace();
    let before = ws.shortcuts().clone();
    let message = ws
        .set_shortcut(MenuAction::Redo, "CmdOrCtrl+Z")
        .expect_err("Undo has it");
    assert!(message.contains("Ctrl") || message.contains("CmdOrCtrl"));
    assert_eq!(ws.shortcuts(), &before);
    assert!(ws.set_shortcut(MenuAction::Redo, "not keys").is_err());
    assert_eq!(ws.shortcuts(), &before);
    assert!(ws.set_shortcut(MenuAction::Redo, "CmdOrCtrl+Alt+K").is_ok());
    assert_eq!(
        shortcut(&ws, MenuAction::Redo).as_deref(),
        Some("CmdOrCtrl+Alt+K")
    );
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
