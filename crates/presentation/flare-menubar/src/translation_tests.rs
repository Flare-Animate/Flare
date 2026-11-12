#![allow(clippy::expect_used)]
//! Every name the menus carry exists in every language, and is not the English one except where
//! the language really uses the same word.

use flare_i18n::{Language, Translator};

use crate::{
    LabelState, MENU_LAYOUT, MenuAction, MenuEntry, all_entries, mock_key, submenu_key, title_key,
};

/// Names a language writes as English does: (language tag, key). Each is a word or a name that
/// the language really shares (a loanword such as Zoom, Lasso, Ellipse, Info, or a word spelled
/// alike such as the German Text and Pause); a test checks the list does not go stale.
const SAME_AS_ENGLISH: &[(&str, &str)] = &[
    ("de", "menu-title-text"),
    ("de", "menu-item-tool-text"),
    ("de", "menu-item-tool-ellipse"),
    ("de", "menu-item-tool-hand"),
    ("de", "menu-item-tool-zoom"),
    ("de", "menu-item-tool-lasso"),
    ("de", "menu-item-show-info"),
    ("de", "menu-item-play-pause-on"),
    ("fr", "menu-item-tool-ellipse"),
    ("fr", "menu-item-tool-zoom"),
    ("fr", "menu-item-tool-rotate-view"),
    ("fr", "menu-item-tool-lasso"),
    ("fr", "menu-help-version"),
    ("fr", "menu-item-tool-rectangle"),
    ("fr", "menu-system-services"),
    ("es", "menu-item-show-color"),
    ("de", "menu-help-version"),
    ("fr", "menu-item-play-pause-on"),
    ("es", "menu-item-tool-zoom"),
    ("es", "menu-title-control"),
    ("pt-BR", "menu-item-tool-zoom"),
    ("de", "menu-item-workspace-animator"),
    ("de", "menu-item-workspace-designer"),
    ("it", "menu-item-workspace-designer"),
    ("pt-BR", "menu-item-workspace-designer"),
];

fn allowed(tag: &str, key: &str) -> bool {
    SAME_AS_ENGLISH.contains(&(tag, key))
}

fn keys() -> Vec<String> {
    let mut keys = Vec::new();
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
        outline_mode: true,
        // Shown is the default, so hidden is the other state.
        pasteboard_shown: false,
        rulers_shown: true,
        grid_shown: true,
        snapping: true,
        loop_playback: true,
        muted: true,
    };
    for action in MenuAction::ALL {
        for state in [LabelState::default(), on] {
            keys.push(action.label_key(&state).to_owned());
        }
    }
    for (title, _) in MENU_LAYOUT {
        keys.push(title_key(title));
    }
    for entry in all_entries() {
        match entry {
            MenuEntry::Submenu(name, _) => keys.push(submenu_key(name)),
            MenuEntry::Mock(name) => keys.push(mock_key(name)),
            _ => {}
        }
    }
    for name in [
        "services",
        "hide",
        "hide-others",
        "show-all",
        "minimize",
        "fullscreen",
        "close-window",
    ] {
        keys.push(format!("menu-system-{name}"));
    }
    keys.extend(["menu-help-version", "menu-help-tagline", "menu-help-alpha"].map(str::to_owned));
    keys.sort();
    keys.dedup();
    keys
}

#[test]
fn every_key_has_english_text_and_a_different_text_in_each_language() {
    let english = Translator::new(Language::English);
    let keys = keys();
    assert!(keys.len() > 150, "{}", keys.len());
    for language in Language::ALL.into_iter().skip(1) {
        let translator = Translator::new(language);
        for key in &keys {
            assert!(english.has(key), "{key} is missing in English");
            assert!(translator.has(key), "{} lacks {key}", language.code());
            if !allowed(language.code(), key) {
                assert_ne!(
                    translator.text(key),
                    english.text(key),
                    "{}: {key} is still English",
                    language.code()
                );
            }
        }
    }
}

#[test]
fn the_allowlist_only_names_keys_that_exist_and_really_are_the_same() {
    let english = Translator::new(Language::English);
    let keys = keys();
    for (tag, key) in SAME_AS_ENGLISH {
        assert!(keys.iter().any(|k| k == key), "{key} is not a menu key");
        let language = Language::from_code(tag).expect("a language");
        assert_eq!(
            Translator::new(language).text(key),
            english.text(key),
            "{tag} {key} differs now: remove it from the list"
        );
    }
}

#[test]
fn the_english_names_are_what_label_returns() {
    let state = LabelState::default();
    assert_eq!(MenuAction::Save.label(&state), "Save");
    assert_eq!(title_key("Commands"), "menu-title-commands");
    assert_eq!(
        mock_key("Manage Saved Commands..."),
        "menu-mock-manage-saved-commands"
    );
}
