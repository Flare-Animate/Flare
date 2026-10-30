//! The menu bar is Animate's: the same menus in the same order, with each command where Animate
//! puts it (Adobe's Help pages: Insert > Timeline, Modify > Timeline, Edit > Timeline, File >
//! Export). A menu Animate does not have is not made up.

use super::*;
use crate::{MENU_LAYOUT, MenuEntry, OFF_MENU, all_entries};

fn menu(title: &str) -> &'static [MenuEntry] {
    MENU_LAYOUT
        .iter()
        .find(|(t, _)| *t == title)
        .unwrap_or_else(|| panic!("no {title} menu"))
        .1
}

/// The labels of the commands directly in a list of entries, with each submenu as `Name >`.
fn shown(ws: &Workspace, entries: &[MenuEntry]) -> Vec<String> {
    entries
        .iter()
        .filter_map(|e| match e {
            MenuEntry::Action(a) => Some(ws.label(*a)),
            MenuEntry::Submenu(name, _) => Some(format!("{name} >")),
            MenuEntry::Mock(name) => Some((*name).to_owned()),
            MenuEntry::Separator => None,
        })
        .collect()
}

fn submenu(entries: &[MenuEntry], name: &str) -> &'static [MenuEntry] {
    entries
        .iter()
        .find_map(|e| match e {
            MenuEntry::Submenu(n, inner) if *n == name => Some(*inner),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no {name} submenu"))
}

#[test]
fn the_menu_bar_has_animates_menus_in_animates_order_and_no_others() {
    let titles: Vec<&str> = MENU_LAYOUT.iter().map(|(t, _)| *t).collect();
    // Animate's bar, without Debug, which is for ActionScript.
    assert_eq!(
        titles,
        [
            "File", "Edit", "View", "Insert", "Modify", "Text", "Commands", "Control", "Window",
            "Help"
        ]
    );
}

#[test]
fn commands_sit_where_animate_keeps_them() {
    let ws = workspace();
    let file = menu("File");
    assert_eq!(
        shown(&ws, submenu(file, "Import")),
        ["Import Sound...", "Import XFL or FLA..."]
    );
    assert_eq!(
        shown(&ws, submenu(file, "Export")),
        ["Export Image...", "Export Animated GIF..."]
    );
    let top = shown(&ws, file);
    assert!(top.contains(&"Publish Settings...".to_owned()) && top.contains(&"Publish".to_owned()));
    let edit = menu("Edit");
    let frames = shown(&ws, submenu(edit, "Timeline"));
    assert!(frames.iter().any(|l| l == "Remove Frames"), "{frames:?}");
    assert!(
        frames.iter().any(|l| l.contains("Copy Frames"))
            && frames.iter().any(|l| l.contains("Paste Frames"))
    );
    let insert = menu("Insert");
    assert_eq!(
        shown(&ws, submenu(insert, "Timeline")),
        [
            "Layer",
            "Layer Folder",
            "Frame",
            "Keyframe",
            "Blank Keyframe"
        ]
    );
    let top = shown(&ws, insert);
    for wanted in ["Create Motion Tween", "Create Shape Tween", "Timeline >"] {
        assert!(
            top.iter().any(|l| l == wanted),
            "{wanted} in Insert: {top:?}"
        );
    }
    let modify = menu("Modify");
    let timeline = shown(&ws, submenu(modify, "Timeline"));
    for wanted in ["Clear Keyframe", "Convert to Keyframes"] {
        assert!(
            timeline.iter().any(|l| l == wanted),
            "{wanted} in Modify > Timeline: {timeline:?}"
        );
    }
    let transform = shown(&ws, submenu(modify, "Transform"));
    assert!(
        transform.iter().any(|l| l == "Flip Horizontal"),
        "{transform:?}"
    );
}

#[test]
fn tools_and_home_have_no_menu_and_nothing_else_is_missing_from_the_menus() {
    let in_menus: Vec<MenuAction> = all_entries()
        .into_iter()
        .filter_map(|e| match e {
            MenuEntry::Action(a) => Some(a),
            _ => None,
        })
        .collect();
    for action in OFF_MENU {
        assert!(!in_menus.contains(action), "{action:?} is in a menu");
        assert!(
            action.tool_key().is_some() || *action == MenuAction::ShowHome,
            "{action:?} is off the menus but is neither a tool nor Home"
        );
    }
    for action in MenuAction::ALL {
        if action.tool_key().is_some() {
            assert!(
                OFF_MENU.contains(&action),
                "{action:?} is a tool and has no key shown elsewhere"
            );
        }
    }
}

#[test]
fn submenus_go_one_level_deep_and_hold_something() {
    for (title, entries) in MENU_LAYOUT {
        for entry in *entries {
            if let MenuEntry::Submenu(name, inner) = entry {
                assert!(!inner.is_empty(), "{title} > {name} is empty");
                assert!(
                    !inner.iter().any(|e| matches!(e, MenuEntry::Submenu(..))),
                    "{title} > {name} holds a menu of its own"
                );
            }
        }
    }
}

#[test]
fn new_layer_from_the_menu_adds_one_as_one_undo_step() {
    let mut ws = workspace();
    assert!(ws.is_enabled(MenuAction::NewLayer));
    ws.perform(MenuAction::NewLayer);
    assert_eq!(layer_count(&ws), 2);
    ws.undo();
    assert_eq!(layer_count(&ws), 1);
}
