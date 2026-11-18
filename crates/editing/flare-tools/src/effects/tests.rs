#![allow(clippy::expect_used)]

use flare_commands::LayerChange;
use flare_fx::{EffectKind, FxError, NodeId, Value};

use super::*;
use crate::test_support::{Fixture, fixture};

fn graph(f: &Fixture) -> Option<EffectGraph> {
    f.session.document().scenes()[0].timeline().layers()[0]
        .effects()
        .cloned()
}

fn fields(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(n, v)| ((*n).to_owned(), (*v).to_owned()))
        .collect()
}

#[test]
fn adding_an_effect_is_one_undo_step_and_shows_at_once() {
    let mut f = fixture();
    let (commands, id) = add(
        f.session.document(),
        f.scene,
        f.layer,
        EffectKind::DropShadow,
        &fields(&[("distance", "12")]),
    )
    .expect("add");
    assert_eq!(id.get(), 3);
    f.session.apply(commands).expect("apply");
    let g = graph(&f).expect("effects");
    assert!(!g.is_identity());
    assert_eq!(
        g.node(id).and_then(|n| n.value("distance")),
        Some(Value::Number(12.0))
    );
    assert!(f.session.undo());
    assert_eq!(graph(&f), None);
}

#[test]
fn every_edit_builds_on_the_graph_the_layer_has() {
    let mut f = fixture();
    let doc = |f: &Fixture| f.session.document().clone();
    let (c, shadow) = add(&doc(&f), f.scene, f.layer, EffectKind::DropShadow, &[]).expect("add");
    f.session.apply(c).expect("apply");
    let (c, glow) = add(&doc(&f), f.scene, f.layer, EffectKind::Glow, &[]).expect("add");
    f.session.apply(c).expect("apply");
    let c = set_fields(
        &doc(&f),
        f.scene,
        f.layer,
        glow,
        &fields(&[("color", "00FF00")]),
    )
    .expect("set");
    f.session.apply(c).expect("apply");
    let c = move_node(&doc(&f), f.scene, f.layer, shadow, [5.0, 90.0]).expect("move");
    f.session.apply(c).expect("apply");
    let c = remove(&doc(&f), f.scene, f.layer, shadow).expect("remove");
    f.session.apply(c).expect("apply");
    let g = graph(&f).expect("effects");
    assert_eq!(g.order(), vec![NodeId::LAYER, glow, NodeId::OUTPUT]);
    let c = disconnect(&doc(&f), f.scene, f.layer, NodeId::OUTPUT, 0).expect("cut");
    f.session.apply(c).expect("apply");
    let c = connect(&doc(&f), f.scene, f.layer, glow, NodeId::OUTPUT, 0).expect("join");
    f.session.apply(c).expect("apply");
    assert_eq!(graph(&f).map(|g| g.order().len()), Some(3));
    let c = clear(&doc(&f), f.scene, f.layer).expect("clear");
    f.session.apply(c).expect("apply");
    assert_eq!(graph(&f), None);
}

#[test]
fn a_change_to_what_is_there_already_is_no_step() {
    let f = fixture();
    assert_eq!(
        clear(f.session.document(), f.scene, f.layer),
        Ok(Vec::new())
    );
    assert_eq!(
        set(
            f.session.document(),
            f.scene,
            f.layer,
            Some(EffectGraph::new())
        ),
        Ok(Vec::new()),
        "the plain graph is no effects"
    );
}

#[test]
fn refusals_name_the_reason_and_build_nothing() {
    let mut f = fixture();
    let doc = f.session.document().clone();
    assert!(matches!(
        add(&doc, f.scene, LayerId::new(), EffectKind::Blur, &[]),
        Err(EffectsRefusal::NoSuchLayer)
    ));
    assert!(matches!(
        add(
            &doc,
            f.scene,
            f.layer,
            EffectKind::Blur,
            &fields(&[("radius", "-1")])
        ),
        Err(EffectsRefusal::Graph(FxError::OutOfRange { .. }))
    ));
    assert!(matches!(
        remove(&doc, f.scene, f.layer, NodeId::OUTPUT),
        Err(EffectsRefusal::Graph(FxError::FixedNode))
    ));
    assert!(matches!(
        kind("output"),
        Err(EffectsRefusal::Graph(FxError::UnknownKind(_)))
    ));
    assert!(matches!(node(0), Err(EffectsRefusal::BadNode(0))));
    f.session
        .apply([Command::UpdateLayer {
            scene: f.scene,
            layer: f.layer,
            change: LayerChange::Mask(true),
        }])
        .expect("mask");
    let masked = f.session.document().clone();
    let refusal = add(&masked, f.scene, f.layer, EffectKind::Blur, &[]).expect_err("refused");
    assert!(refusal.to_string().contains("mask"), "{refusal}");
}

#[test]
fn a_port_is_found_by_number_or_by_name() {
    let mut f = fixture();
    let (c, over) = add(
        f.session.document(),
        f.scene,
        f.layer,
        EffectKind::Over,
        &[],
    )
    .expect("add");
    f.session.apply(c).expect("apply");
    let doc = f.session.document();
    assert_eq!(port(doc, f.scene, f.layer, over, "under"), Ok(1));
    assert_eq!(port(doc, f.scene, f.layer, over, "0"), Ok(0));
    let refusal = port(doc, f.scene, f.layer, over, "top").expect_err("no top");
    assert!(refusal.to_string().contains("over, under"), "{refusal}");
    assert!(
        port(doc, f.scene, f.layer, NodeId::LAYER, "0").is_err(),
        "the Layer takes nothing"
    );
}

#[test]
fn a_graph_and_the_kinds_read_as_json() {
    assert_eq!(describe(None), serde_json::Value::Null);
    let (g, id) = EffectGraph::new().append(EffectKind::Snow).expect("add");
    let json = describe(Some(&g));
    assert_eq!(json["animated"], true);
    assert_eq!(json["changes_picture"], true);
    let snow = &json["nodes"][2];
    assert_eq!(snow["id"], id.get());
    assert_eq!(snow["kind"], "snow");
    assert_eq!(snow["inputs"][0]["name"], "under");
    assert_eq!(snow["inputs"][0]["from"], 1);
    assert_eq!(snow["settings"]["color"], "#FFFFFFFF");
    let listed = kinds();
    let names: Vec<&str> = listed
        .as_array()
        .expect("list")
        .iter()
        .filter_map(|k| k["kind"].as_str())
        .collect();
    assert!(names.contains(&"drop-shadow") && names.contains(&"snow"));
    assert!(!names.contains(&"layer") && !names.contains(&"output"));
}
