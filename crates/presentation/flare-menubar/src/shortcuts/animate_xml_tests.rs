use super::*;
use crate::Accelerator;

/// A file as Animate writes it, with the bindings given.
fn file(bindings: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<keyboardshortcuts>{bindings}</keyboardshortcuts>"
    )
}

fn keycmd(cmd: &str, key: &str, mods: &[&str]) -> String {
    let mods: String = mods.iter().map(|m| format!("<{m}/>")).collect();
    format!("<keycmd><cmd>{cmd}</cmd>{key}<mod>{mods}</mod></keycmd>")
}

#[test]
fn typed_keys_function_keys_and_modifiers_become_flare_shortcuts() {
    let xml = file(
        &[
            keycmd("File.New", "<uchar>N</uchar>", &["cmdkey"]),
            keycmd("Edit.Redo", "<uchar>Y</uchar>", &["cmdkey"]),
            keycmd(
                "File.PublishSettings",
                "<key>18</key>",
                &["shiftkey", "cmdkey"],
            ),
            keycmd("Window.Output", "<key>8</key>", &[]),
            keycmd("Arrange.BringForward", "<key>44</key>", &["cmdkey"]),
            keycmd("Tool.Rotate", "<uchar>R</uchar>", &[]),
            keycmd("Tool.Brush", "<uchar>B</uchar>", &[]),
            keycmd(
                "Edit.PasteInPlace",
                "<uchar>V</uchar>",
                &["cmdkey", "shiftkey"],
            ),
        ]
        .concat(),
    );
    let import = ShortcutMap::from_animate_xml(&xml).expect("reads");
    let keys = |action| import.map.get(action).map(ToString::to_string);
    assert_eq!(keys(A::New).as_deref(), Some("CmdOrCtrl+N"));
    assert_eq!(
        keys(A::PublishSettings).as_deref(),
        Some("CmdOrCtrl+Shift+F12")
    );
    assert_eq!(keys(A::ShowOutput).as_deref(), Some("F2"));
    assert_eq!(keys(A::BringForward).as_deref(), Some("CmdOrCtrl+Up"));
    assert_eq!(keys(A::ToolRotateView).as_deref(), Some("R"));
    assert_eq!(keys(A::ToolBrush).as_deref(), Some("B"));
    assert_eq!(keys(A::PasteInPlace).as_deref(), Some("CmdOrCtrl+Shift+V"));
    assert_eq!(import.map.set(), "animate");
    assert_eq!(import.applied.len(), 8);
    assert!(import.skipped.is_empty(), "{:?}", import.skipped);
}

#[test]
fn a_key_the_file_gives_is_taken_from_the_command_that_had_it_in_animates_set() {
    let base = ShortcutMap::from_set("animate").expect("set");
    let holder = base
        .entries()
        .into_iter()
        .find(|(a, accel)| *a != A::ToolRotateView && accel.to_string() == "R");
    let xml = file(&keycmd("Tool.Rotate", "<uchar>R</uchar>", &[]));
    let import = ShortcutMap::from_animate_xml(&xml).expect("reads");
    assert_eq!(
        import
            .map
            .owner(&Accelerator::parse("R", A::ToolRotateView).expect("r")),
        Some(A::ToolRotateView)
    );
    if let Some((previous, _)) = holder {
        assert!(import.map.get(previous).is_none(), "{previous:?} gave R up");
    }
}

#[test]
fn what_flare_cannot_use_is_skipped_with_the_reason_and_the_rest_is_kept() {
    let xml = file(
        &[
            keycmd("Debug.StepIn", "<key>12</key>", &["altkey"]),
            keycmd("File.Save", "<key>99</key>", &["cmdkey"]),
            keycmd("File.Open", "<uchar>O</uchar>", &["hyperkey"]),
            keycmd("Tool.Rotate", "<uchar>R</uchar>", &[]),
            keycmd("Tool.Rotate", "<uchar>H</uchar>", &["shiftkey"]),
            keycmd("Edit.Copy", "<uchar>C</uchar>", &[]),
            keycmd("File.Close", "", &["cmdkey"]),
            keycmd("File.New", "<uchar>N</uchar>", &["cmdkey"]),
        ]
        .concat(),
    );
    let import = ShortcutMap::from_animate_xml(&xml).expect("reads");
    let reason = |command: &str, keys: &str| {
        import
            .skipped
            .iter()
            .find(|s| s.command == command && s.keys.contains(keys))
            .map(|s| s.reason.clone())
            .unwrap_or_else(|| panic!("{command} {keys} skipped: {:?}", import.skipped))
    };
    assert!(reason("Debug.StepIn", "").contains("no such command"));
    assert!(reason("File.Save", "99").contains("not known yet"));
    assert!(reason("File.Open", "O").contains("hyperkey"));
    assert!(reason("Tool.Rotate", "H").contains("one shortcut"));
    assert!(reason("Edit.Copy", "C").contains("bare key"));
    assert!(reason("File.Close", "").contains("no key"));
    assert_eq!(import.skipped.len(), 6);
    assert_eq!(
        import.applied,
        vec![
            (A::ToolRotateView, "R".to_owned()),
            (A::New, "CmdOrCtrl+N".to_owned())
        ]
    );
    // The skipped command keeps Animate's own key.
    let base = ShortcutMap::from_set("animate").expect("set");
    assert_eq!(import.map.get(A::Save), base.get(A::Save));
}

#[test]
fn escaped_characters_are_read_as_the_keys_they_name() {
    let xml = file(
        &[
            keycmd("View.ZoomIn", "<uchar>&#61;</uchar>", &["cmdkey"]),
            keycmd("View.ZoomOut", "<uchar>-</uchar>", &["cmdkey"]),
            keycmd("Edit.Clear", "<uchar>&lt;</uchar>", &[]),
        ]
        .concat(),
    );
    let import = ShortcutMap::from_animate_xml(&xml).expect("reads");
    assert_eq!(
        import
            .map
            .get(A::ZoomIn)
            .map(ToString::to_string)
            .as_deref(),
        Some("CmdOrCtrl+=")
    );
    assert_eq!(
        import
            .map
            .get(A::ZoomOut)
            .map(ToString::to_string)
            .as_deref(),
        Some("CmdOrCtrl+-")
    );
    assert_eq!(import.skipped.len(), 1, "`<` is no key Flare has");
}

#[test]
fn damaged_or_hostile_files_are_refused_and_build_nothing() {
    assert!(matches!(
        ShortcutMap::from_animate_xml(&" ".repeat(MAX_ANIMATE_SHORTCUT_BYTES + 1)),
        Err(ShortcutError::TooLarge(_))
    ));
    for bad in [
        "<keyboardshortcuts><keycmd><cmd>File.New</cmd>",
        "<a></b>",
        "<!DOCTYPE x [<!ENTITY a \"b\">]><x>&a;</x>",
        "<x>&nope;</x>",
        &format!("{}{}", "<a>".repeat(40), "</a>".repeat(40)),
        &file(&keycmd("File.New", "<uchar>N</uchar>", &["cmdkey"]).repeat(MAX_BINDINGS + 1)),
    ] {
        assert!(
            matches!(
                ShortcutMap::from_animate_xml(bad),
                Err(ShortcutError::BadFile(_))
            ),
            "{}",
            &bad[..bad.len().min(60)]
        );
    }
}

#[test]
fn every_command_in_the_table_is_named_once_and_maps_to_a_distinct_flare_command() {
    let mut ids = std::collections::HashSet::new();
    let mut actions = std::collections::HashSet::new();
    for (id, action) in COMMANDS {
        assert!(ids.insert(*id), "{id} twice");
        assert!(actions.insert(*action), "{action:?} twice");
        assert!(id.contains('.'), "{id} is Family.Command");
    }
}

#[test]
fn the_outcome_reports_what_was_applied_skipped_and_the_file_to_keep() {
    let xml = file(
        &[
            keycmd("File.New", "<uchar>N</uchar>", &["cmdkey"]),
            keycmd("Debug.Continue", "<key>11</key>", &["altkey"]),
        ]
        .concat(),
    );
    let out = ShortcutMap::from_animate_xml(&xml)
        .expect("reads")
        .to_json();
    assert_eq!(out["applied"][0]["command"], "New");
    assert_eq!(out["skipped"][0]["command"], "Debug.Continue");
    assert_eq!(out["set"], "animate");
    let kept = ShortcutMap::from_json(&out["file"].to_string()).expect("a shortcut file");
    assert_eq!(
        kept.get(A::New).map(ToString::to_string).as_deref(),
        Some("CmdOrCtrl+N")
    );
}
