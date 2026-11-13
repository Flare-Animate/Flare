#![allow(clippy::expect_used)]
use super::*;
use crate::MenuAction as A;

#[test]
fn every_set_loads_without_conflict_and_differs_from_flare() {
    let flare = ShortcutMap::from_set("flare").expect("flare");
    for id in SHORTCUT_SET_IDS {
        let map = ShortcutMap::from_set(id).unwrap_or_else(|e| panic!("{id}: {e}"));
        assert!(map.conflicts().is_empty(), "{id}");
        assert_eq!(map.set(), id);
        if id == "opentoonz" || id == "tahoma" {
            assert_ne!(map, flare, "{id}");
        }
    }
}

#[test]
fn flare_set_matches_the_builtin_table() {
    let map = ShortcutMap::default();
    for action in A::ALL {
        let want = action.shortcut().or_else(|| action.tool_key());
        assert_eq!(map.get(action).map(|a| a.to_string()).as_deref(), want);
    }
}

#[test]
fn profile_set_ids_all_have_tables() {
    for id in flare_profile::SHORTCUT_SETS {
        assert!(SHORTCUT_SET_IDS.contains(&id), "{id}");
    }
    assert!(ShortcutMap::from_set("nope").is_err());
}

#[test]
fn spellings_of_one_chord_are_equal() {
    let a = Accelerator::parse("shift+ctrl+z", A::Undo).expect("a");
    let b = Accelerator::parse("CmdOrCtrl+Shift+Z", A::Undo).expect("b");
    assert_eq!(a, b);
    assert_eq!(a.to_string(), "CmdOrCtrl+Shift+Z");
}

#[test]
fn bad_accelerators_are_refused() {
    for text in [
        "",
        "Ctrl+",
        "Ctrl+Ctrl+Z",
        "Hyper+Z",
        "F13",
        "Ctrl+ZZ",
        "+",
        "A+B",
    ] {
        assert!(Accelerator::parse(text, A::Undo).is_err(), "{text}");
    }
    assert!(matches!(
        Accelerator::parse("Q", A::Undo),
        Err(ShortcutError::BareKey(_))
    ));
    assert!(Accelerator::parse("Q", A::ToolPen).is_ok());
}

#[test]
fn a_conflicting_bind_changes_nothing() {
    let mut map = ShortcutMap::default();
    let before = map.clone();
    let err = map.bind(A::Redo, "CmdOrCtrl+Z").expect_err("conflict");
    assert!(matches!(err, ShortcutError::Conflict { ref with, .. } if with == "Undo"));
    assert_eq!(map, before);
    assert!(map.bind(A::Redo, "bad key").is_err());
    assert_eq!(map, before);
}

#[test]
fn binding_over_keys_takes_them_and_frees_their_command() {
    let mut map = ShortcutMap::default();
    let taken = map.bind_taking(A::Redo, "CmdOrCtrl+Z").expect("taken");
    assert_eq!(taken, Some(A::Undo));
    assert_eq!(
        map.get(A::Redo).map(|a| a.to_string()).as_deref(),
        Some("CmdOrCtrl+Z")
    );
    assert!(map.get(A::Undo).is_none(), "Undo gave its keys up");
    assert!(map.conflicts().is_empty());
    // The freed command stays free through a save and a load.
    let loaded = ShortcutMap::from_json(&map.to_json()).expect("load");
    assert_eq!(loaded, map);
    // Its own keys again take nothing; keys that are not a shortcut still change nothing.
    assert_eq!(map.bind_taking(A::Redo, "CmdOrCtrl+Z").expect("same"), None);
    let before = map.clone();
    assert!(map.bind_taking(A::Redo, "bad key").is_err());
    assert!(
        map.bind_taking(A::Save, "S").is_err(),
        "a bare letter is a tool's"
    );
    assert_eq!(map, before);
    // Reset takes the set's own keys back from whoever holds them.
    assert_eq!(map.reset_taking(A::Undo).expect("reset"), Some(A::Redo));
    assert!(map.get(A::Redo).is_none());
    assert!(map.conflicts().is_empty());
}

#[test]
fn bind_unbind_reset() {
    let mut map = ShortcutMap::default();
    map.bind(A::Save, "CmdOrCtrl+Alt+S").expect("bind");
    assert_eq!(
        map.get(A::Save).map(|a| a.to_string()).as_deref(),
        Some("CmdOrCtrl+Alt+S")
    );
    map.reset(A::Save).expect("reset");
    assert_eq!(map, ShortcutMap::default());
    map.unbind(A::Open);
    assert!(map.get(A::Open).is_none());
    // The freed key can go to another command.
    map.bind(A::Close, "CmdOrCtrl+O").expect("freed key");
}

#[test]
fn json_round_trips_only_the_differences() {
    let mut map = ShortcutMap::from_set("opentoonz").expect("set");
    map.bind(A::Save, "CmdOrCtrl+Alt+S").expect("bind");
    map.unbind(A::Quit);
    let text = map.to_json();
    assert!(!text.contains("\"Open\""));
    assert_eq!(ShortcutMap::from_json(&text).expect("load"), map);
    let fresh = ShortcutMap::from_set("flare").expect("flare");
    assert_eq!(
        ShortcutMap::from_json(&fresh.to_json()).expect("load"),
        fresh
    );
}

#[test]
fn a_swap_in_a_file_is_not_a_conflict() {
    let text = r#"{"schema":1,"set":"flare","bindings":{"Undo":"CmdOrCtrl+Shift+Z","Redo":"CmdOrCtrl+Z"}}"#;
    let map = ShortcutMap::from_json(text).expect("swap");
    assert_eq!(
        map.get(A::Undo).map(|a| a.to_string()).as_deref(),
        Some("CmdOrCtrl+Shift+Z")
    );
}

#[test]
fn bad_files_are_refused_and_fall_back() {
    for text in [
        "not json",
        r#"{"schema":2,"set":"flare","bindings":{}}"#,
        r#"{"schema":1,"set":"zzz","bindings":{}}"#,
        r#"{"schema":1,"set":"flare","bindings":{"Nope":"F5"}}"#,
        r#"{"schema":1,"set":"flare","bindings":{"Close":"CmdOrCtrl+O"}}"#,
        r#"{"schema":1,"set":"flare","bindings":{"Close":"Q"}}"#,
        r#"{"schema":1,"set":"flare","bindings":{},"extra":1}"#,
    ] {
        assert!(ShortcutMap::from_json(text).is_err(), "{text}");
        let (map, why) = ShortcutMap::from_json_or_default(text);
        assert_eq!(map, ShortcutMap::default());
        assert!(why.is_some());
    }
    let huge = " ".repeat(MAX_SHORTCUT_FILE_BYTES + 1);
    assert!(matches!(
        ShortcutMap::from_json(&huge),
        Err(ShortcutError::TooLarge(_))
    ));
}

#[test]
fn the_muda_text_of_every_default_shortcut_is_the_table_the_native_menu_registers() {
    let map = ShortcutMap::default();
    for action in MenuAction::ALL {
        if action.tool_key().is_some() {
            continue;
        }
        assert_eq!(
            map.get(action).map(Accelerator::muda_text).as_deref(),
            action.shortcut(),
            "{action:?}"
        );
    }
}
