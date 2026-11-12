#![allow(clippy::expect_used)]

use super::MenuEntry::Action;
use super::*;

#[test]
fn every_action_appears_in_the_layout_exactly_once() {
    let seen: Vec<MenuAction> = all_entries()
        .into_iter()
        .filter_map(|entry| match entry {
            Action(action) => Some(action),
            _ => None,
        })
        .collect();
    for action in MenuAction::ALL {
        let in_menus = seen.iter().filter(|a| **a == action).count();
        let off_menu = OFF_MENU.contains(&action);
        assert_eq!(
            in_menus,
            usize::from(!off_menu),
            "{action:?}: in the menus {in_menus} times, off the menus: {off_menu}"
        );
    }
    assert_eq!(seen.len() + OFF_MENU.len(), MenuAction::ALL.len());
}

#[test]
fn ids_round_trip_and_all_is_complete() {
    for action in MenuAction::ALL {
        assert_eq!(MenuAction::from_id(&action.id()), Some(action));
    }
    assert_eq!(MenuAction::from_id("NoSuchAction"), None);
}

#[test]
fn shortcuts_are_unique_never_a_bare_letter_and_read_naturally() {
    let mut seen = std::collections::HashSet::new();
    for action in MenuAction::ALL {
        if let Some(shortcut) = action.shortcut() {
            assert!(seen.insert(shortcut), "{shortcut} is used twice");
            let key = shortcut.rsplit('+').next().unwrap_or_default();
            let bare = !shortcut.contains('+');
            assert!(
                !(bare && key.len() == 1),
                "{action:?}: a bare letter {shortcut}"
            );
            let text = action.shortcut_text().expect("text");
            assert!(!text.contains("CmdOrCtrl"), "{text}");
        }
    }
    assert!(MenuAction::Open.shortcut_text().is_some());
    assert!(MenuAction::Delete.shortcut().is_none());
}

#[test]
fn tool_keys_are_unique_letters_and_every_menu_icon_exists() {
    let mut keys = std::collections::HashSet::new();
    for action in MenuAction::ALL {
        if let Some(key) = action.tool_key() {
            assert!(keys.insert(key), "{key} is used twice");
            assert_eq!(key.len(), 1);
        }
        if let Some(icon) = action.icon() {
            assert!(
                flare_ui::icons::exists(icon),
                "{action:?} names a missing icon {icon}"
            );
        }
    }
    assert!(keys.len() >= 6);
}

mod labels {
    use super::*;
    use crate::LabelState;

    #[test]
    fn every_action_has_a_name_in_both_states_of_what_it_toggles() {
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
                let label = action.label(&state);
                assert!(!label.trim().is_empty(), "{action:?} has no name");
            }
        }
    }

    #[test]
    fn a_toggling_item_names_what_it_will_do_next() {
        let off = LabelState::default();
        let on = LabelState {
            playing: true,
            layer_locked: true,
            muted: true,
            ..off
        };
        assert_eq!(MenuAction::PlayPause.label(&off), "Play");
        assert_eq!(MenuAction::PlayPause.label(&on), "Pause");
        assert_eq!(MenuAction::ToggleLayerLocked.label(&off), "Lock Layer");
        assert_eq!(MenuAction::ToggleLayerLocked.label(&on), "Unlock Layer");
        assert_eq!(MenuAction::ToggleMute.label(&off), "Mute Sounds");
        assert_eq!(MenuAction::ToggleMute.label(&on), "Unmute Sounds");
        // A layer with nothing to ask about is taken to be shown, so the item offers to hide it.
        assert_eq!(MenuAction::ToggleLayerVisible.label(&off), "Hide Layer");
    }

    #[test]
    fn items_that_do_not_toggle_have_one_name_whatever_the_state() {
        let on = LabelState {
            playing: true,
            muted: true,
            text_bold: true,
            ..LabelState::default()
        };
        for action in [
            MenuAction::Open,
            MenuAction::Save,
            MenuAction::ToolPen,
            MenuAction::Undo,
        ] {
            assert_eq!(action.label(&LabelState::default()), action.label(&on));
        }
        assert_eq!(MenuAction::Preferences.label(&on), "Preferences...");
    }
}
