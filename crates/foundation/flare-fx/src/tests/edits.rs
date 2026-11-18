//! Editing a graph: each edit gives a new checked graph, and a refused one changes nothing.

use flare_core::Rgba8;

use crate::{EffectGraph, EffectKind, FxError, NodeId, Value, parse_color, parse_fields};

#[test]
fn appending_puts_the_effect_into_the_chain_before_the_output() {
    let (graph, shadow) = EffectGraph::new()
        .append(EffectKind::DropShadow)
        .expect("add");
    assert_eq!(graph.order(), vec![NodeId::LAYER, shadow, NodeId::OUTPUT]);
    let (graph, glow) = graph.append(EffectKind::Glow).expect("add");
    assert_eq!(
        graph.order(),
        vec![NodeId::LAYER, shadow, glow, NodeId::OUTPUT]
    );
    let output = graph.node(NodeId::OUTPUT).expect("output");
    assert!(
        output.at()[0] > graph.node(glow).expect("glow").at()[0],
        "room in the view"
    );
}

#[test]
fn an_appended_merge_takes_the_chain_under_it() {
    let (graph, over) = EffectGraph::new().append(EffectKind::Over).expect("add");
    let node = graph.node(over).expect("over");
    assert_eq!(node.inputs(), &[None, Some(NodeId::LAYER)]);
}

#[test]
fn removing_an_effect_rejoins_the_chain() {
    let (graph, shadow) = EffectGraph::new()
        .append(EffectKind::DropShadow)
        .expect("add");
    let (graph, glow) = graph.append(EffectKind::Glow).expect("add");
    let graph = graph.remove(shadow).expect("remove");
    assert_eq!(graph.order(), vec![NodeId::LAYER, glow, NodeId::OUTPUT]);
    let graph = graph.remove(glow).expect("remove");
    assert!(graph.is_identity());
    assert_eq!(graph.remove(NodeId::LAYER), Err(FxError::FixedNode));
    assert_eq!(graph.remove(NodeId::OUTPUT), Err(FxError::FixedNode));
    assert_eq!(
        graph.remove(NodeId::new(40).expect("id")),
        Err(FxError::NoSuchNode(40))
    );
}

#[test]
fn connecting_into_a_loop_is_refused_and_changes_nothing() {
    let (graph, a) = EffectGraph::new().append(EffectKind::Blur).expect("add");
    let (graph, b) = graph.append(EffectKind::Glow).expect("add");
    let before = graph.clone();
    assert_eq!(graph.connect(b, a, 0), Err(FxError::Cycle));
    assert_eq!(graph, before);
    assert_eq!(
        graph.connect(NodeId::OUTPUT, a, 0),
        Err(FxError::FromOutput)
    );
    assert!(matches!(
        graph.connect(a, b, 3),
        Err(FxError::NoSuchPort {
            port: 3,
            ports: 1,
            ..
        })
    ));
}

#[test]
fn disconnecting_the_output_leaves_nothing_drawn() {
    let graph = EffectGraph::new()
        .disconnect(NodeId::OUTPUT, 0)
        .expect("disconnect");
    assert!(!graph.is_identity());
    assert_eq!(graph.order(), vec![NodeId::OUTPUT]);
}

#[test]
fn the_fixed_nodes_cannot_be_added() {
    assert_eq!(
        EffectGraph::new().add(EffectKind::Layer, [0.0; 2]),
        Err(FxError::FixedNode)
    );
    assert_eq!(
        EffectGraph::new().append(EffectKind::Output),
        Err(FxError::FixedNode)
    );
}

#[test]
fn settings_are_set_by_name_from_text_all_or_none() {
    let (graph, shadow) = EffectGraph::new()
        .append(EffectKind::DropShadow)
        .expect("add");
    let fields =
        parse_fields(&["distance=20", "color=#FF000080", " angle = -90 "]).expect("fields");
    let set = graph.set_fields(shadow, &fields).expect("set");
    let node = set.node(shadow).expect("node");
    assert_eq!(node.value("distance"), Some(Value::Number(20.0)));
    assert_eq!(node.value("angle"), Some(Value::Number(-90.0)));
    assert_eq!(
        node.value("color"),
        Some(Value::Color(Rgba8::new(255, 0, 0, 128)))
    );

    let bad = parse_fields(&["distance=30", "opacity=150"]).expect("fields");
    assert!(matches!(
        graph.set_fields(shadow, &bad),
        Err(FxError::OutOfRange {
            name: "opacity",
            ..
        })
    ));
    let unknown = parse_fields(&["sparkle=1"]).expect("fields");
    assert!(matches!(
        graph.set_fields(shadow, &unknown),
        Err(FxError::UnknownParam { .. })
    ));
    let word = parse_fields(&["distance=far"]).expect("fields");
    assert!(matches!(
        graph.set_fields(shadow, &word),
        Err(FxError::NotANumber { .. })
    ));
    let color = parse_fields(&["color=red"]).expect("fields");
    assert!(matches!(
        graph.set_fields(shadow, &color),
        Err(FxError::BadColor { .. })
    ));
    assert!(matches!(
        parse_fields(&["distance"]),
        Err(FxError::BadField(_))
    ));
}

#[test]
fn a_count_is_rounded_to_a_whole_number() {
    let (graph, snow) = EffectGraph::new().append(EffectKind::Snow).expect("add");
    let graph = graph
        .set_value(snow, "flakes", Value::Number(12.6))
        .expect("set");
    assert_eq!(
        graph.node(snow).and_then(|n| n.value("flakes")),
        Some(Value::Number(13.0))
    );
}

#[test]
fn colors_read_with_or_without_alpha_and_a_hash() {
    assert_eq!(
        parse_color("#102030"),
        Some(Rgba8::opaque(0x10, 0x20, 0x30))
    );
    assert_eq!(
        parse_color("10203040"),
        Some(Rgba8::new(0x10, 0x20, 0x30, 0x40))
    );
    assert_eq!(parse_color("1020"), None);
    assert_eq!(parse_color("zz2030"), None);
    assert_eq!(parse_color("€€€"), None);
}
