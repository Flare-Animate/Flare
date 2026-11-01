//! Renaming the scene by double clicking its name in the bar above the stage.

use super::layer_names::{click, find, settle};
use super::*;
use egui::PointerButton;

/// A workspace that has drawn a few passes, with where the scene's name sits in the bar found.
fn settled() -> (egui::Context, Workspace, Pos2, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    rect(&mut ws);
    let mut time = 0.0;
    let mut at = None;
    for _ in 0..4 {
        time += 0.1;
        at = find(&mut ws, &ctx, time, vec![], "Scene 1").or(at);
    }
    (ctx, ws, at.expect("the scene's name is drawn"), time)
}

fn scene_name(ws: &Workspace) -> String {
    ws.document().scenes()[0].name().to_owned()
}

fn editing(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, at: Pos2) {
    for _ in 0..2 {
        click(ws, ctx, time, at, PointerButton::Primary);
    }
    settle(ws, ctx, time);
}

#[test]
fn double_clicking_the_scene_name_edits_it_in_place_and_enter_keeps_it_as_one_undo_step() {
    let (ctx, mut ws, at, mut time) = settled();
    click(&mut ws, &ctx, &mut time, at, PointerButton::Primary);
    assert!(ws.scene_renaming.is_none(), "one click only");
    click(&mut ws, &ctx, &mut time, at, PointerButton::Primary);
    settle(&mut ws, &ctx, &mut time);
    let text = ws
        .scene_renaming
        .as_mut()
        .expect("the name is being edited");
    assert_eq!(text, "Scene 1");
    *text = "  Intro ".to_owned();
    time += 0.1;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::Enter, Modifiers::NONE)]),
    );
    settle(&mut ws, &ctx, &mut time);
    assert!(ws.scene_renaming.is_none());
    assert_eq!(scene_name(&ws), "Intro", "trimmed");
    ws.undo();
    assert_eq!(scene_name(&ws), "Scene 1", "one undo step");
    ws.redo();
    assert_eq!(scene_name(&ws), "Intro");
}

#[test]
fn escape_keeps_the_old_name_and_a_blank_name_is_refused_with_a_message() {
    let (ctx, mut ws, at, mut time) = settled();
    editing(&mut ws, &ctx, &mut time, at);
    *ws.scene_renaming.as_mut().expect("editing") = "Nope".to_owned();
    time += 0.1;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    assert!(ws.scene_renaming.is_none());
    assert_eq!(scene_name(&ws), "Scene 1");

    ws.begin_scene_rename();
    settle(&mut ws, &ctx, &mut time);
    *ws.scene_renaming.as_mut().expect("editing again") = "   ".to_owned();
    time += 0.1;
    pass(
        &mut ws,
        &ctx,
        input(time, vec![key(Key::Enter, Modifiers::NONE)]),
    );
    assert_eq!(scene_name(&ws), "Scene 1", "a blank name changes nothing");
    assert!(ws.status().is_some_and(|s| s.contains("needs a name")));
}

#[test]
fn the_same_name_is_no_edit_and_a_name_that_is_too_long_is_refused() {
    let (_, mut ws, _, _) = settled();
    let revision = ws.active().revision();
    ws.rename_scene("Scene 1");
    assert_eq!(ws.active().revision(), revision, "nothing to undo");
    ws.rename_scene(&"x".repeat(flare_commands::MAX_SCENE_NAME_CHARS + 1));
    assert_eq!(scene_name(&ws), "Scene 1");
    assert!(ws.status().is_some(), "the refusal is shown");
    assert_eq!(ws.active().revision(), revision);
}

#[test]
fn inside_a_symbol_the_scene_cannot_be_renamed() {
    let (_, mut ws, _, _) = settled();
    ws.new_symbol();
    let id = ws.document().symbols()[0].id();
    assert!(ws.enter_symbol(id));
    assert!(ws.in_symbol());
    ws.begin_scene_rename();
    assert!(ws.scene_renaming.is_none());
    ws.rename_scene("Elsewhere");
    assert!(ws.exit_symbol());
    assert_eq!(scene_name(&ws), "Scene 1");
}
