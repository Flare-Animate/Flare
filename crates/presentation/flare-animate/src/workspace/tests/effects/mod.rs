//! The Effects panel: the active layer's effect graph, edited by the panel's buttons and by
//! dragging on its graph view, each change one undo step, a held setting shown on the stage
//! before it is applied.

use flare_fx::{EffectKind, NodeId, Value};

use super::*;

mod graph_view;

fn effects(ws: &Workspace) -> Option<flare_fx::EffectGraph> {
    ws.document().scenes()[0].timeline().layers()[0]
        .effects()
        .cloned()
}

fn steps(ws: &Workspace) -> usize {
    ws.active().undo_labels().len()
}

#[test]
fn the_panel_is_in_the_strip_and_the_window_menu_brings_it_up() {
    let mut ws = workspace();
    assert!(ws.panels.layout.in_strip(Pane::Effects));
    ws.perform(MenuAction::ShowEffects);
    assert!(ws.pane_is_front(Pane::Effects));
}

#[test]
fn every_name_the_panel_builds_has_a_message() {
    for kind in EffectKind::ALL {
        assert!(
            flare_i18n::has_key(&format!("fx-kind-{}", kind.name())),
            "{kind:?}"
        );
        for input in kind.inputs() {
            assert!(flare_i18n::has_key(&format!("fx-input-{input}")), "{input}");
        }
        for spec in kind.params() {
            assert!(
                flare_i18n::has_key(&format!("fx-param-{}", spec.name)),
                "{}",
                spec.name
            );
        }
    }
    assert!(Workspace::HISTORY_LABELS.iter().any(|(label, _)| *label
        == super::super::selection::step_label(super::super::effects::EFFECTS_STEP)));
}

#[test]
fn adding_removing_and_clearing_are_one_undo_step_each() {
    let mut ws = workspace();
    rect(&mut ws);
    let before = steps(&ws);
    let shadow = ws.add_layer_effect(EffectKind::DropShadow).expect("added");
    assert_eq!(steps(&ws), before + 1);
    assert_eq!(ws.effects.selected, Some(shadow), "the new node is chosen");
    let glow = ws.add_layer_effect(EffectKind::Glow).expect("added");
    let graph = effects(&ws).expect("effects");
    assert_eq!(
        graph.order(),
        vec![NodeId::LAYER, shadow, glow, NodeId::OUTPUT]
    );
    ws.remove_layer_effect(shadow);
    assert_eq!(effects(&ws).expect("effects").order().len(), 3);
    ws.clear_layer_effects();
    assert_eq!(effects(&ws), None);
    assert_eq!(steps(&ws), before + 4);
    ws.undo();
    assert!(effects(&ws).is_some());
}

#[test]
fn a_held_setting_is_shown_on_the_stage_and_applied_once_when_let_go() {
    let mut ws = workspace();
    rect(&mut ws);
    let shadow = ws.add_layer_effect(EffectKind::DropShadow).expect("added");
    let before = steps(&ws);
    for distance in [20.0, 30.0, 40.0] {
        ws.hold_layer_effect_value(shadow, "distance", Value::Number(distance));
    }
    assert_eq!(steps(&ws), before, "nothing applied while held");
    let (_, shown) = ws
        .previewed_document()
        .expect("the stage shows the held value");
    let held = shown.scenes()[0].timeline().layers()[0]
        .effects()
        .and_then(|g| g.node(shadow))
        .and_then(|n| n.value("distance"));
    assert_eq!(held, Some(Value::Number(40.0)));
    // A pass in which the panel held nothing lets it go.
    ws.settle_effects(10);
    assert_eq!(steps(&ws), before + 1, "one step for the whole drag");
    assert!(ws.previewed_document().is_none());
    let applied = effects(&ws).and_then(|g| g.node(shadow).and_then(|n| n.value("distance")));
    assert_eq!(applied, Some(Value::Number(40.0)));
}

#[test]
fn a_setting_held_while_the_panel_holds_it_is_not_applied_yet() {
    let mut ws = workspace();
    rect(&mut ws);
    let glow = ws.add_layer_effect(EffectKind::Glow).expect("added");
    let before = steps(&ws);
    ws.hold_layer_effect_value(glow, "radius", Value::Number(30.0));
    ws.effects.held_pass = Some(9);
    ws.settle_effects(10);
    assert_eq!(steps(&ws), before, "still held in the last pass");
    ws.settle_effects(11);
    assert_eq!(steps(&ws), before + 1);
}

#[test]
fn a_held_setting_from_before_an_undo_is_dropped() {
    let mut ws = workspace();
    rect(&mut ws);
    let glow = ws.add_layer_effect(EffectKind::Glow).expect("added");
    ws.hold_layer_effect_value(glow, "radius", Value::Number(30.0));
    ws.undo();
    let after_undo = effects(&ws);
    ws.release_layer_effects();
    assert_eq!(
        effects(&ws),
        after_undo,
        "the undone document is not overwritten"
    );
}

#[test]
fn a_bad_value_or_a_layer_that_draws_nothing_is_refused_with_a_message() {
    let mut ws = workspace();
    rect(&mut ws);
    let blur = ws.add_layer_effect(EffectKind::Blur).expect("added");
    ws.hold_layer_effect_value(blur, "radius", Value::Number(-4.0));
    assert!(ws.effects.held.is_none());
    assert!(
        ws.status().is_some_and(|s| s.contains("radius")),
        "{:?}",
        ws.status()
    );

    let layer = ws.active_layer_id().expect("layer");
    ws.clear_layer_effects();
    ws.toggle_layer_mask(layer);
    assert!(ws.add_layer_effect(EffectKind::Glow).is_none());
    assert!(
        ws.status().is_some_and(|s| s.contains("mask")),
        "{:?}",
        ws.status()
    );
}

#[test]
fn inside_a_symbol_there_is_no_layer_for_effects() {
    let (mut ws, _) = with_symbol();
    ws.edit_selected_symbol();
    assert!(ws.in_symbol());
    assert!(ws.effects_layer().is_none());
    assert!(ws.add_layer_effect(EffectKind::Glow).is_none());
}
