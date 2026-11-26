//! The Game panel: an entity is added, given a component and removed, gravity is set, and a run
//! reports; each edit is one undo step and what the library refuses is said and changes nothing.

use super::*;
use flare_game::ComponentKind;

fn entities(ws: &Workspace) -> usize {
    ws.document().game().map_or(0, |g| g.entities().count())
}

#[test]
fn an_entity_is_added_given_a_component_and_removed_each_in_one_undo_step() {
    let mut ws = workspace();
    ws.game_add_entity(" Ball ");
    assert_eq!(entities(&ws), 1);
    assert!(ws.game.selected.is_some(), "the new entity is chosen");
    ws.game_set_component(ComponentKind::ALL[0], "");
    let first = ws
        .document()
        .game()
        .expect("game")
        .entities()
        .next()
        .expect("entity");
    assert_eq!(first.name(), "Ball", "the name is trimmed");
    ws.undo();
    assert_eq!(entities(&ws), 1, "the component's undo leaves the entity");
    ws.game_remove_entity();
    assert_eq!(entities(&ws), 0);
    assert!(ws.game.selected.is_none());
    ws.undo();
    assert_eq!(entities(&ws), 1, "one undo brings it back");
}

#[test]
fn gravity_is_one_step_and_a_bad_value_or_field_is_refused_with_the_game_unchanged() {
    let mut ws = workspace();
    ws.game_add_entity("Ball");
    ws.game_set_gravity(0.0, 300.0);
    assert_eq!(ws.document().game().expect("game").gravity(), (0.0, 300.0));
    let revision = ws.active().revision();
    ws.game_set_gravity(f64::NAN, 0.0);
    assert_eq!(ws.active().revision(), revision);
    assert!(ws.status().is_some_and(|s| s.contains("cannot")));
    ws.game_set_component(ComponentKind::ALL[0], "nonsense");
    assert_eq!(
        ws.active().revision(),
        revision,
        "a bad field changes nothing"
    );
}

#[test]
fn a_run_says_what_happened_and_a_project_with_no_game_says_so() {
    let mut ws = workspace();
    ws.game_run();
    assert!(ws.game.last_run.is_none());
    assert!(ws.status().is_some_and(|s| s.contains("no game")));
    ws.game_add_entity("Ball");
    ws.game.frames = 10;
    ws.game_run();
    assert!(
        ws.game
            .last_run
            .as_deref()
            .is_some_and(|s| s.contains("10 frames"))
    );
}

#[test]
fn a_scripted_run_moves_the_world_and_a_bad_script_is_refused_unchanged() {
    let mut ws = workspace();
    ws.game_add_entity("Ball");
    for kind in [
        ComponentKind::Transform,
        ComponentKind::Body,
        ComponentKind::Control,
    ] {
        ws.game_set_component(kind, "");
    }
    ws.game.frames = 5;
    ws.game_run();
    let plain = ws.game.last_run.clone().expect("ran");
    ws.game.script = "right".into();
    ws.game_run();
    let scripted = ws.game.last_run.clone().expect("ran");
    assert_ne!(plain, scripted, "the script changes the run");

    ws.game.script = "not-a-key".into();
    ws.game_run();
    assert_eq!(
        ws.game.last_run.as_deref(),
        Some(scripted.as_str()),
        "a refused script changes nothing"
    );
    assert!(ws.status().is_some_and(|s| s.contains("cannot")));
}

#[test]
fn the_panel_draws_empty_and_with_an_entity_chosen() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    ws.perform(MenuAction::ShowGame);
    run(&mut ws, &ctx, 0.0);
    ws.game_add_entity("Ball");
    ws.game_run();
    ws.perform(MenuAction::ShowGame);
    run(&mut ws, &ctx, 1.0);
    run(&mut ws, &ctx, 2.0);
    assert_eq!(entities(&ws), 1);
}
