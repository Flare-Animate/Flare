use super::*;
use crate::arrange::find;

fn essentials() -> WorkspaceId {
    WorkspaceId::Preset(Preset::Essentials)
}

#[test]
fn every_preset_places_every_panel_once_and_they_differ() {
    let mut seen = Vec::new();
    for preset in Preset::ALL {
        let layout = preset.layout();
        layout::check(&layout).unwrap_or_else(|e| panic!("{preset:?}: {e}"));
        // The arrangement without the tree's own id, which every tree has differently.
        let mut json: serde_json::Value =
            serde_json::from_str(&layout::to_json(&layout).expect("json")).expect("json");
        if let Some(tree) = json.get_mut("tree").and_then(|t| t.as_object_mut()) {
            tree.remove("id");
        }
        assert!(!seen.contains(&json), "{preset:?} is like another");
        seen.push(json);
    }
    assert!(Preset::SmallScreen.layout().strip.is_empty());
    assert_eq!(
        Preset::Essentials.layout().strip,
        layout::default_layout().strip
    );
    // Designer has Properties left of the stage; Essentials has it on the right.
    let side_of_stage = |layout: &Layout| {
        let tree = &layout.tree;
        let stage_column = tree
            .tiles
            .parent_of(find(tree, Pane::Stage).expect("stage"))
            .expect("column");
        let row = tree.tiles.parent_of(stage_column).expect("row");
        let Some(egui_tiles::Tile::Container(container)) = tree.tiles.get(row) else {
            panic!("a row");
        };
        let children: Vec<_> = container.children().copied().collect();
        let mut tile = find(tree, Pane::Properties).expect("docked");
        while !children.contains(&tile) {
            tile = tree.tiles.parent_of(tile).expect("inside the row");
        }
        let at = |t| children.iter().position(|c| *c == t);
        at(tile) < at(stage_column)
    };
    assert!(side_of_stage(&Preset::Designer.layout()), "left");
    assert!(!side_of_stage(&Preset::Essentials.layout()), "right");
}

#[test]
fn each_workspace_keeps_its_own_changes_and_reset_forgets_them() {
    let mut workspaces = Workspaces::default();
    let mut current = Preset::Essentials.layout();
    current.dock(Pane::Color);
    current = workspaces
        .switch(current, WorkspaceId::Preset(Preset::Animator))
        .expect("switches");
    assert_eq!(workspaces.active, WorkspaceId::Preset(Preset::Animator));
    assert_eq!(current.strip, Preset::Animator.layout().strip);
    let back = workspaces.switch(current, essentials()).expect("switches");
    assert!(!back.in_strip(Pane::Color), "Essentials as it was left");
    let reset = workspaces.reset();
    assert!(reset.in_strip(Pane::Color), "as built in");
    // After a reset, coming back finds it as built in, not as it was left.
    let animator = workspaces
        .switch(reset, WorkspaceId::Preset(Preset::Animator))
        .expect("switches");
    let again = workspaces.switch(animator, essentials()).expect("switches");
    assert!(again.in_strip(Pane::Color));
}

#[test]
fn a_saved_workspace_is_switched_to_replaced_by_name_and_deleted() {
    let mut workspaces = Workspaces::default();
    let mut mine = Preset::Essentials.layout();
    mine.collapse(Pane::Library).expect("collapses");
    workspaces
        .save_as("  Inking ", mine.clone())
        .expect("saved");
    let id = WorkspaceId::Custom("Inking".into());
    assert_eq!(workspaces.active, id, "trimmed, and now in use");
    assert_eq!(workspaces.custom_names().collect::<Vec<_>>(), ["Inking"]);

    let mut changed = mine.clone();
    changed.dock(Pane::Library);
    let essentials_layout = workspaces.switch(changed, essentials()).expect("switch");
    let left = workspaces
        .switch(essentials_layout, id.clone())
        .expect("back");
    assert!(!left.in_strip(Pane::Library), "as it was left");
    assert!(
        workspaces.reset().in_strip(Pane::Library),
        "reset is as saved"
    );

    // The same name in another case replaces it rather than adding one.
    workspaces
        .save_as("INKING", Preset::Classic.layout())
        .expect("replaced");
    assert_eq!(workspaces.custom_names().count(), 1);

    let shown = workspaces.delete("INKING").expect("deleted");
    assert!(shown.is_some(), "it was in use, so Essentials comes back");
    assert_eq!(workspaces.active, essentials());
    assert!(workspaces.custom_names().next().is_none());
    assert!(workspaces.delete("INKING").is_err(), "gone");
    assert!(
        workspaces
            .switch(
                Preset::Essentials.layout(),
                WorkspaceId::Custom("nope".into())
            )
            .is_err()
    );
    assert_eq!(workspaces.active, essentials(), "a refusal changes nothing");
}

#[test]
fn bad_names_and_too_many_workspaces_are_refused() {
    let mut workspaces = Workspaces::default();
    let layout = Preset::Essentials.layout();
    for bad in ["", "   ", "tab\there", &"x".repeat(MAX_NAME_CHARS + 1)] {
        assert!(workspaces.save_as(bad, layout.clone()).is_err(), "{bad:?}");
    }
    for n in 0..MAX_CUSTOM {
        workspaces
            .save_as(&format!("w{n}"), layout.clone())
            .expect("within the bound");
    }
    assert!(workspaces.save_as("one more", layout.clone()).is_err());
    assert!(
        workspaces.save_as("w3", layout).is_ok(),
        "replacing is fine"
    );
}

#[test]
fn workspaces_round_trip_and_older_files_open_as_essentials() {
    let mut workspaces = Workspaces::default();
    let mut mine = Preset::Designer.layout();
    mine.collapse(Pane::Color).expect("collapses");
    workspaces.save_as("Mine", mine.clone()).expect("saved");
    let mut current = mine.clone();
    current.dock(Pane::Color);
    let json = workspaces_to_json(&workspaces, &current).expect("json");
    let (opened, shown) = workspaces_from_json(&json).expect("opens");
    assert_eq!(opened.active, WorkspaceId::Custom("Mine".into()));
    assert!(!shown.in_strip(Pane::Color), "the arrangement on screen");
    assert_eq!(opened.custom_names().collect::<Vec<_>>(), ["Mine"]);

    let layout_only = layout::to_json(&mine).expect("json");
    let (opened, shown) = workspaces_from_json(&layout_only).expect("opens");
    assert_eq!(opened.active, essentials());
    assert!(shown.in_strip(Pane::Color));
    // A bare tree, as the earliest files are, holds every panel.
    let every: Vec<Pane> = Pane::ALL.into_iter().filter(|p| p.can_collapse()).collect();
    let bare = serde_json::to_string(&crate::arrangement(&every)).expect("json");
    let (opened, shown) = workspaces_from_json(&bare).expect("opens");
    assert_eq!(opened.active, essentials());
    assert!(shown.strip.is_empty());
    // A tree missing side panels opens with them in the strip; one missing the stage is refused.
    let partial = serde_json::to_string(&mine.tree).expect("json");
    let (_, shown) = workspaces_from_json(&partial).expect("opens");
    assert!(shown.in_strip(Pane::Color));
    let mut no_stage = mine.tree.clone();
    assert!(crate::arrange::remove(&mut no_stage, Pane::Stage));
    let no_stage = serde_json::to_string(&no_stage).expect("json");
    assert!(
        workspaces_from_json(&no_stage).is_err(),
        "the stage is missing"
    );
}

#[test]
fn damaged_workspace_files_are_refused() {
    let mut workspaces = Workspaces::default();
    workspaces
        .save_as("Mine", Preset::Essentials.layout())
        .expect("saved");
    let json = workspaces_to_json(&workspaces, &Preset::Essentials.layout()).expect("json");
    let huge = " ".repeat(MAX_WORKSPACES_BYTES + 1);
    assert!(matches!(
        workspaces_from_json(&huge),
        Err(LayoutError::TooLarge)
    ));
    // Active names a workspace that was never saved.
    let missing = json.replace(
        "\"active\":{\"custom\":\"Mine\"}",
        "\"active\":{\"custom\":\"Other\"}",
    );
    assert_ne!(missing, json, "the test edits the file");
    assert!(workspaces_from_json(&missing).is_err());
    // A saved layout with the stage twice.
    let broken = json.replace("\"Timeline\"", "\"Stage\"");
    assert!(workspaces_from_json(&broken).is_err());
    // The same name saved twice.
    let mut value: serde_json::Value = serde_json::from_str(&json).expect("json");
    let first = value["custom"][0].clone();
    value["custom"].as_array_mut().expect("array").push(first);
    assert!(workspaces_from_json(&value.to_string()).is_err());
}

#[test]
fn saved_workspaces_from_before_a_panel_existed_open_with_it_in_the_strip() {
    let mut workspaces = Workspaces::default();
    let mut old = Preset::Classic.layout();
    old.strip.retain(|p| *p != Pane::Swatches);
    workspaces.save_as("Old", old.clone()).expect("saved");
    let json = workspaces_to_json(&workspaces, &old).expect("json");
    let (opened, shown) = workspaces_from_json(&json).expect("opens");
    assert!(shown.in_strip(Pane::Swatches));
    let mut opened = opened;
    assert!(opened.reset().in_strip(Pane::Swatches), "the saved one too");
}
