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
