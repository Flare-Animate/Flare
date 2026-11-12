use super::*;

fn names(map: &ShortcutMap, query: &str) -> Vec<MenuAction> {
    let state = LabelState::default();
    map.search(query, |a| a.label(&state))
}

#[test]
fn words_find_commands_by_name_id_or_key_in_any_case() {
    let map = ShortcutMap::default();
    let found = names(&map, "save");
    assert!(found.contains(&MenuAction::Save) && found.contains(&MenuAction::SaveAs));
    assert!(!found.contains(&MenuAction::Open));
    // Every word must be found: "save as" is narrower than "save".
    let narrower = names(&map, "  SAVE   as ");
    assert!(narrower.contains(&MenuAction::SaveAs));
    assert!(!narrower.contains(&MenuAction::Save));
    // A key on its own finds what it runs.
    assert_eq!(
        names(&map, "f5"),
        [MenuAction::InsertFrame, MenuAction::RemoveFrame]
    );
    // A modifier's name finds the chords that hold it, whatever it is called.
    for word in ["option", "alt", "\u{2325}"] {
        let held = names(&map, word);
        assert!(held.contains(&MenuAction::CopyFrames), "{word}");
        assert!(!held.contains(&MenuAction::Save), "{word}");
    }
    // The tool letters are listed and found too, the Rotation tool among them.
    assert!(names(&map, "rotation").contains(&MenuAction::ToolRotateView));
    assert!(names(&map, "nothing has this name").is_empty());
}

#[test]
fn keys_find_exactly_that_chord() {
    let map = ShortcutMap::default();
    for query in ["cmd+s", "Ctrl+S", "\u{2318}S", "CmdOrCtrl + s"] {
        assert_eq!(names(&map, query), [MenuAction::Save], "{query}");
    }
    assert_eq!(names(&map, "\u{2318}\u{21e7}s"), [MenuAction::SaveAs]);
    assert_eq!(names(&map, "shift+f5"), [MenuAction::RemoveFrame]);
    // Modifiers alone: every chord holding them.
    let with_alt = names(&map, "cmd+alt+");
    assert!(with_alt.contains(&MenuAction::CopyFrames));
    assert!(!with_alt.contains(&MenuAction::Save));
    // Two keys are no chord.
    assert!(names(&map, "s+o").is_empty());
}

#[test]
fn the_search_follows_the_map_in_force() {
    let mut map = ShortcutMap::default();
    map.bind(MenuAction::Open, "CmdOrCtrl+Shift+O")
        .expect("bind");
    assert_eq!(names(&map, "cmd+shift+o"), [MenuAction::Open]);
    assert!(
        names(&map, "cmd+o").is_empty(),
        "its old keys find nothing now"
    );
    // A freed command keeps its row, found by name though it has no keys.
    map.unbind(MenuAction::Open);
    assert!(names(&map, "open").contains(&MenuAction::Open));
}

#[test]
fn nothing_typed_lists_every_row_and_long_queries_are_cut() {
    let map = ShortcutMap::default();
    let all = names(&map, "");
    assert_eq!(
        all.len(),
        MenuAction::ALL
            .into_iter()
            .filter(|a| map.listed(*a))
            .count()
    );
    assert_eq!(ShortcutQuery::parse("   "), ShortcutQuery::All);
    let long = "s".repeat(10_000);
    let ShortcutQuery::Words(words) = ShortcutQuery::parse(&long) else {
        panic!("words");
    };
    assert_eq!(words[0].chars().count(), MAX_SEARCH_CHARS);
}

#[test]
fn the_json_search_has_english_names_and_only_what_matched() {
    let map = ShortcutMap::default();
    let out = map.search_json("save as");
    assert_eq!(out["search"], "save as");
    let rows = out["shortcuts"].as_array().expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["action"], MenuAction::SaveAs.id());
    assert_eq!(rows[0]["shortcut"], "CmdOrCtrl+Shift+S");
    assert_eq!(out["set"], "flare");
}
