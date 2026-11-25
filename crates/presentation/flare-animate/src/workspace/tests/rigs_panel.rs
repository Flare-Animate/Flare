//! The Rigs panel: a symbol is given a rig, bones and poses, each edit one undo step, and what the
//! library refuses is said and changes nothing.

use super::*;
use flare_document::Symbol;
use flare_rig::Bone;

fn with_symbol() -> (Workspace, SymbolId) {
    let mut ws = workspace();
    let id = SymbolId::new();
    ws.active_mut()
        .apply([flare_commands::Command::AddSymbol(Symbol::new(
            id,
            "Arm",
            Timeline::new(vec![]),
        ))])
        .expect("symbol");
    (ws, id)
}

fn rig_of(ws: &Workspace, id: SymbolId) -> Option<flare_rig::Rig> {
    flare_editor::rig_library::find(ws.document(), id).expect("a valid rig")
}

fn bone(name: &str, parent: Option<usize>) -> Bone {
    Bone {
        name: name.to_owned(),
        parent,
        length: 10.0,
        rest_angle: 0.0,
        limit: None,
        target: None,
    }
}

#[test]
fn a_rig_gets_bones_and_poses_each_in_one_undo_step() {
    let (mut ws, id) = with_symbol();
    assert_eq!(ws.rig_symbol(), None, "nothing chosen yet");
    ws.rig_create(id);
    assert_eq!(ws.rig_symbol(), Some(id), "the new rig is chosen");
    ws.rig_add_bone(id, bone("upper", None));
    ws.rig_add_bone(id, bone("lower", Some(0)));
    ws.rig_set_pose(id, 4, 1, 0.5);
    ws.rig_set_pose(id, 4, 0, -0.25);
    let rig = rig_of(&ws, id).expect("rig");
    assert_eq!(rig.bones.len(), 2);
    assert_eq!(
        rig.keys[&4].pose.rotations.len(),
        2,
        "a key keeps its other bones"
    );
    ws.rig_remove_pose(id, 4);
    assert!(rig_of(&ws, id).expect("rig").keys.is_empty());
    ws.undo();
    assert_eq!(
        rig_of(&ws, id).expect("rig").keys.len(),
        1,
        "one undo brings the pose back"
    );
    ws.rig_remove(id);
    assert!(rig_of(&ws, id).is_none());
    ws.undo();
    assert!(rig_of(&ws, id).is_some(), "and the rig");
}

#[test]
fn a_bad_edit_is_said_and_changes_nothing() {
    let (mut ws, id) = with_symbol();
    ws.rig_add_bone(id, bone("x", None));
    assert!(
        ws.status().is_some_and(|s| s.contains("no rig")),
        "{:?}",
        ws.status()
    );
    ws.rig_create(id);
    let revision = ws.active().revision();
    ws.rig_add_bone(id, bone("orphan", Some(7)));
    assert_eq!(ws.active().revision(), revision);
    ws.rig_set_pose(id, 0, 9, 1.0);
    assert_eq!(ws.active().revision(), revision, "no such bone");
    ws.rig_create(id);
    assert_eq!(ws.active().revision(), revision, "already rigged");
}

#[test]
fn the_panel_draws_empty_and_with_a_rig_and_the_menu_opens_it() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.perform(MenuAction::ShowRigs);
    run(&mut ws, &ctx, 0.0);
    let id = SymbolId::new();
    ws.active_mut()
        .apply([flare_commands::Command::AddSymbol(
            flare_document::Symbol::new(id, "Arm", Timeline::new(vec![])),
        )])
        .expect("symbol");
    ws.rig_create(id);
    ws.rig_add_bone(id, bone("upper", None));
    ws.perform(MenuAction::ShowRigs);
    run(&mut ws, &ctx, 1.0);
    run(&mut ws, &ctx, 2.0);
}
