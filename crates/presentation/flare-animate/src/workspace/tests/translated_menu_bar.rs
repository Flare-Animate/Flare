//! The menu bar and the Help windows in each language: titles drawn in the bar, every menu item's
//! label (both names of a toggling one), the layout's menus inside menus and planned commands,
//! and the About and shortcuts windows.

use super::*;
use flare_menubar::LabelState;

fn drawn_text(ws: &mut Workspace) -> Vec<String> {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut all = Vec::new();
    for step in 0..5 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| ws.show(ui));
        out.textures_delta.clear();
        all = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
    }
    all
}

fn speaking(tag: &str) -> Workspace {
    let mut ws = workspace();
    ws.set_system_locale(tag);
    ws
}

#[test]
fn the_menu_bar_draws_its_titles_in_the_language() {
    for (tag, titles) in [
        (
            "de-DE",
            ["Datei", "Bearbeiten", "Ansicht", "Fenster", "Hilfe"],
        ),
        (
            "fr-FR",
            ["Fichier", "Édition", "Affichage", "Fenêtre", "Aide"],
        ),
        (
            "ja-JP",
            ["ファイル", "編集", "表示", "ウィンドウ", "ヘルプ"],
        ),
        ("zh-CN", ["文件", "编辑", "视图", "窗口", "帮助"]),
    ] {
        let text = drawn_text(&mut speaking(tag));
        for title in titles {
            assert!(text.iter().any(|t| t == title), "{tag} {title}: {text:?}");
        }
        for english in ["File", "Edit", "View", "Window", "Help"] {
            assert!(
                !text.iter().any(|t| t == english),
                "{tag}: {english} is English"
            );
        }
    }
}

#[test]
fn toggling_items_are_named_in_both_states_in_the_language() {
    let mut ws = speaking("de-DE");
    assert_eq!(ws.label(MenuAction::PlayPause), "Wiedergabe");
    assert_eq!(ws.label(MenuAction::ToggleLayerLocked), "Ebene sperren");
    ws.perform(MenuAction::ToggleLayerLocked);
    assert_eq!(ws.label(MenuAction::ToggleLayerLocked), "Ebene entsperren");
    assert_eq!(ws.label(MenuAction::ToggleMute), "Ton stummschalten");
    ws.perform(MenuAction::ToggleMute);
    assert_eq!(ws.label(MenuAction::ToggleMute), "Ton einschalten");

    // The same state in Japanese says the other thing, so the language, not the state, decides.
    ws.set_system_locale("ja-JP");
    assert_eq!(ws.label(MenuAction::ToggleMute), "サウンドのミュートを解除");
    ws.set_system_locale("en-US");
    assert_eq!(ws.label(MenuAction::ToggleMute), "Unmute Sounds");
}

#[test]
fn every_action_menu_title_and_planned_command_is_translated_in_every_language() {
    let on = LabelState {
        motion_tween: true,
        shape_tween: true,
        playing: true,
        layer_visible: false,
        layer_locked: true,
        layer_mask: true,
        layer_guide: true,
        text_bold: true,
        text_italic: true,
        guides_shown: true,
        onion_on: true,
        multiple_frames: true,
        outline_mode: true,
        // Shown is the default, so hidden is the other state.
        pasteboard_shown: false,
        rulers_shown: true,
        grid_shown: true,
        snapping: true,
        loop_playback: true,
        muted: true,
    };
    for tag in ["de", "es", "fr", "pt-BR", "ja", "ko", "zh-CN"] {
        let ws = speaking(tag);
        for action in MenuAction::ALL {
            for state in [LabelState::default(), on] {
                let key = action.label_key(&state);
                let said = ws.tr(key);
                assert_ne!(said, key, "{tag}: {key} has no text");
                assert!(!said.trim().is_empty(), "{tag}: {key}");
            }
        }
        for (title, entries) in MENU_LAYOUT {
            let said = ws.menu_title(title);
            assert!(
                !said.starts_with("menu-") && !said.is_empty(),
                "{tag} {title}"
            );
            for entry in *entries {
                match entry {
                    MenuEntry::Submenu(name, _) => {
                        assert_ne!(ws.submenu_title(name), *name, "{tag} {name}");
                    }
                    MenuEntry::Mock(name) => {
                        let said = ws.mock_label(name);
                        assert!(!said.starts_with("menu-"), "{tag} {name}: {said}");
                        assert!(!said.trim().is_empty(), "{tag} {name}");
                    }
                    _ => {}
                }
            }
        }
    }
}

#[test]
fn the_shortcut_list_and_the_help_windows_speak_the_language() {
    let mut ws = speaking("es-ES");
    let lines = ws.shortcut_lines();
    assert!(
        lines.iter().any(|(name, _)| name == "Guardar como..."),
        "{lines:?}"
    );
    ws.perform(MenuAction::About);
    ws.perform(MenuAction::KeyboardShortcuts);
    let text = drawn_text(&mut ws);
    for wanted in [
        "Acerca de Flare",
        "Atajos de teclado",
        "Alfa temprana: conserva copias de seguridad de tu trabajo.",
    ] {
        assert!(text.iter().any(|t| t == wanted), "{wanted}: {text:?}");
    }
    assert!(
        text.iter().any(|t| t.starts_with("Versión ")),
        "the version line: {text:?}"
    );
    for english in [
        "About Flare",
        "Keyboard Shortcuts",
        "Early alpha: keep backups of your work.",
    ] {
        assert!(!text.iter().any(|t| t == english), "{english}: {text:?}");
    }
}
