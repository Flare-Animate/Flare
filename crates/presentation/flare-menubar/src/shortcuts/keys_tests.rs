use super::*;
use crate::MenuAction as A;

fn texts(map: &ShortcutMap, action: A) -> Vec<String> {
    map.keys(action).iter().map(ToString::to_string).collect()
}

#[test]
fn a_command_can_have_several_keys_and_the_first_is_the_menus() {
    let mut map = ShortcutMap::default();
    map.add_key(A::Undo, "CmdOrCtrl+Alt+Z").expect("free");
    map.add_key(A::Undo, "F9").expect("free");
    assert_eq!(
        texts(&map, A::Undo),
        ["CmdOrCtrl+Z", "CmdOrCtrl+Alt+Z", "F9"]
    );
    assert_eq!(
        map.get(A::Undo).map(ToString::to_string).as_deref(),
        Some("CmdOrCtrl+Z")
    );
    for key in ["CmdOrCtrl+Z", "CmdOrCtrl+Alt+Z", "F9"] {
        let accel = Accelerator::parse(key, A::Undo).expect("key");
        assert_eq!(map.owner(&accel), Some(A::Undo), "{key}");
    }
    let rows = map
        .entries()
        .into_iter()
        .filter(|(a, _)| *a == A::Undo)
        .count();
    assert_eq!(rows, 3, "one entry for each key");
    assert!(
        map.all_text(A::Undo)
            .is_some_and(|t| t.matches(", ").count() == 2)
    );
    assert!(map.conflicts().is_empty());
    // A key it has already changes nothing.
    let before = map.clone();
    map.add_key(A::Undo, "f9").expect("same key");
    assert_eq!(map, before);
}

#[test]
fn adding_is_refused_whole_for_a_conflict_a_bad_key_or_too_many() {
    let mut map = ShortcutMap::default();
    let before = map.clone();
    assert!(matches!(
        map.add_key(A::Redo, "CmdOrCtrl+Z"),
        Err(ShortcutError::Conflict { ref with, .. }) if with == "Undo"
    ));
    assert!(
        map.add_key(A::Redo, "Q").is_err(),
        "a bare letter is a tool's"
    );
    assert_eq!(map, before);
    for n in 1..MAX_KEYS_PER_COMMAND {
        map.add_key(A::Undo, &format!("CmdOrCtrl+Alt+{n}"))
            .expect("room");
    }
    let full = map.clone();
    assert_eq!(
        map.add_key(A::Undo, "CmdOrCtrl+Alt+Shift+8"),
        Err(ShortcutError::TooManyKeys(MAX_KEYS_PER_COMMAND))
    );
    assert_eq!(
        map.add_key_taking(A::Undo, "CmdOrCtrl+S"),
        Err(ShortcutError::TooManyKeys(MAX_KEYS_PER_COMMAND))
    );
    assert_eq!(map, full, "nothing taken from Save either");
}

#[test]
fn taking_one_key_leaves_the_others_and_the_last_frees_the_command() {
    let mut map = ShortcutMap::default();
    map.add_key(A::Redo, "F9").expect("free");
    let first = map.get(A::Redo).cloned().expect("Redo's own");
    assert_eq!(map.add_key_taking(A::Undo, "F9"), Ok(Some(A::Redo)));
    assert_eq!(
        map.keys(A::Redo),
        std::slice::from_ref(&first),
        "Redo keeps its first"
    );
    assert_eq!(
        map.bind_taking(A::Undo, &first.to_string()),
        Ok(Some(A::Redo))
    );
    assert!(
        map.keys(A::Redo).is_empty(),
        "its last key gone, Redo is free"
    );
    assert_eq!(
        texts(&map, A::Undo),
        [first.to_string()],
        "bind replaces every key"
    );
    assert!(map.conflicts().is_empty());
}

#[test]
fn removing_one_key_makes_the_next_the_first() {
    let mut map = ShortcutMap::default();
    map.add_key(A::Save, "CmdOrCtrl+Alt+Shift+7").expect("free");
    let own = Accelerator::parse("CmdOrCtrl+S", A::Save).expect("key");
    assert!(map.remove_key(A::Save, &own));
    assert_eq!(texts(&map, A::Save), ["CmdOrCtrl+Alt+Shift+7"]);
    assert!(!map.remove_key(A::Save, &own), "not there any more");
    let seven = Accelerator::parse("CmdOrCtrl+Alt+Shift+7", A::Save).expect("key");
    assert!(map.remove_key(A::Save, &seven));
    assert!(map.get(A::Save).is_none());
    // Reset brings the set's own key back alone.
    map.reset(A::Save).expect("free again");
    assert_eq!(texts(&map, A::Save), ["CmdOrCtrl+S"]);
}

#[test]
fn several_keys_round_trip_through_the_file_as_a_list() {
    let mut map = ShortcutMap::default();
    map.add_key(A::ToolBrush, "Shift+B").expect("free");
    map.add_key(A::Save, "CmdOrCtrl+Alt+Shift+7").expect("free");
    map.unbind(A::Undo);
    let text = map.to_json();
    let value: serde_json::Value = serde_json::from_str(&text).expect("json");
    assert_eq!(
        value["bindings"]["ToolBrush"],
        serde_json::json!(["B", "Shift+B"])
    );
    assert_eq!(value["bindings"]["Undo"], serde_json::Value::Null);
    assert!(
        value["bindings"].get("Redo").is_none(),
        "unchanged is left out"
    );
    assert_eq!(ShortcutMap::from_json(&text).expect("reads"), map);
    // One key is still written, and read, as plain text.
    let one = r#"{"schema":1,"set":"flare","bindings":{"Save":"CmdOrCtrl+Alt+Shift+7"}}"#;
    let read = ShortcutMap::from_json(one).expect("reads");
    assert_eq!(texts(&read, A::Save), ["CmdOrCtrl+Alt+Shift+7"]);
}

#[test]
fn a_file_with_too_many_keys_or_one_key_twice_builds_nothing() {
    let many = r#"{"schema":1,"set":"flare","bindings":{"Save":["F1","F2","F3","F4","F5"]}}"#;
    assert_eq!(
        ShortcutMap::from_json(many),
        Err(ShortcutError::TooManyKeys(MAX_KEYS_PER_COMMAND))
    );
    let twice = r#"{"schema":1,"set":"flare","bindings":{"Save":["CmdOrCtrl+Alt+Shift+7"],"Open":["CmdOrCtrl+O","CmdOrCtrl+Alt+Shift+7"]}}"#;
    assert!(matches!(
        ShortcutMap::from_json(twice),
        Err(ShortcutError::Conflict { .. })
    ));
    // A swap of keys between two commands is not a conflict.
    let swap = r#"{"schema":1,"set":"flare","bindings":{"Undo":["CmdOrCtrl+Shift+Z"],"Redo":"CmdOrCtrl+Z"}}"#;
    assert!(ShortcutMap::from_json(swap).is_ok());
}

#[test]
fn a_listing_gives_every_key_and_a_search_finds_a_command_by_its_second_key() {
    let mut map = ShortcutMap::default();
    map.add_key(A::Save, "CmdOrCtrl+Alt+Shift+7").expect("free");
    let list = map.list_json();
    let save = list["shortcuts"]
        .as_array()
        .and_then(|rows| rows.iter().find(|r| r["action"] == "Save"))
        .cloned()
        .expect("Save is listed");
    assert_eq!(save["shortcut"], "CmdOrCtrl+S");
    assert_eq!(
        save["keys"],
        serde_json::json!(["CmdOrCtrl+S", "CmdOrCtrl+Alt+Shift+7"])
    );
    let found = map.search("cmd+alt+shift+7", |a| a.id());
    assert!(found.contains(&A::Save), "{found:?}");
}

#[test]
fn an_edit_by_command_id_does_what_it_names_or_changes_nothing() {
    let mut map = ShortcutMap::default();
    let add = ShortcutEdit::Add("CmdOrCtrl+Alt+Shift+7".to_owned());
    assert_eq!(map.edit("Save", &add), Ok(None));
    assert_eq!(
        texts(&map, A::Save),
        ["CmdOrCtrl+S", "CmdOrCtrl+Alt+Shift+7"]
    );
    let remove = ShortcutEdit::Remove("CmdOrCtrl+S".to_owned());
    assert_eq!(map.edit("Save", &remove), Ok(None));
    assert_eq!(texts(&map, A::Save), ["CmdOrCtrl+Alt+Shift+7"]);
    let before = map.clone();
    assert!(matches!(
        map.edit("Save", &remove),
        Err(ShortcutError::NotBound { .. })
    ));
    assert!(matches!(
        map.edit("NoSuchCommand", &ShortcutEdit::Clear),
        Err(ShortcutError::UnknownAction(_))
    ));
    assert_eq!(map, before);
    let set = ShortcutEdit::Set("CmdOrCtrl+Z".to_owned());
    assert_eq!(map.edit("Save", &set), Ok(Some(A::Undo)));
    assert_eq!(map.edit("Undo", &ShortcutEdit::Reset), Ok(Some(A::Save)));
    assert_eq!(map.edit("Save", &ShortcutEdit::Clear), Ok(None));
    assert!(map.keys(A::Save).is_empty());
}
