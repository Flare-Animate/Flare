//! What every graph must be: the fixed two nodes, inputs that fit, no loops, settings in range.

use flare_core::Rgba8;

use crate::{EffectGraph, EffectKind, FxError, MAX_NODES, Node, NodeId, Value};

fn id(raw: u32) -> NodeId {
    NodeId::new(raw).expect("id")
}

fn node(raw: u32, kind: EffectKind, inputs: Vec<Option<u32>>) -> Node {
    let base = Node::new(id(raw), kind, [0.0, 0.0]);
    Node::from_parts(
        id(raw),
        kind,
        inputs.into_iter().map(|i| i.map(id)).collect(),
        base.values().to_vec(),
        [0.0, 0.0],
    )
}

fn layer() -> Node {
    node(1, EffectKind::Layer, vec![])
}

#[test]
fn a_new_graph_draws_the_layer_as_it_is() {
    let graph = EffectGraph::new();
    assert!(graph.is_identity());
    assert_eq!(graph.order(), vec![NodeId::LAYER, NodeId::OUTPUT]);
    assert!(!graph.is_animated());
    assert_eq!(graph.reach(), 0.0);
}

#[test]
fn the_layer_and_output_are_required_and_fixed_to_their_numbers() {
    let output = node(2, EffectKind::Output, vec![Some(1)]);
    assert_eq!(
        EffectGraph::from_nodes(vec![output.clone()]),
        Err(FxError::FixedNodes)
    );
    let second_layer = node(3, EffectKind::Layer, vec![]);
    assert_eq!(
        EffectGraph::from_nodes(vec![layer(), output.clone(), second_layer]),
        Err(FxError::FixedNodes)
    );
    let swapped = node(1, EffectKind::Blur, vec![None]);
    assert_eq!(
        EffectGraph::from_nodes(vec![swapped, output]),
        Err(FxError::FixedNodes)
    );
}

#[test]
fn inputs_must_fit_the_kind_and_name_nodes_that_exist() {
    let output = node(2, EffectKind::Output, vec![Some(3)]);
    let over = node(3, EffectKind::Over, vec![Some(1)]);
    assert!(matches!(
        EffectGraph::from_nodes(vec![layer(), output.clone(), over]),
        Err(FxError::WrongInputs {
            expected: 2,
            got: 1,
            ..
        })
    ));
    let dangling = node(3, EffectKind::Blur, vec![Some(9)]);
    assert_eq!(
        EffectGraph::from_nodes(vec![layer(), output.clone(), dangling]),
        Err(FxError::DanglingInput { node: 3, from: 9 })
    );
    let from_output = node(3, EffectKind::Blur, vec![Some(2)]);
    assert_eq!(
        EffectGraph::from_nodes(vec![layer(), output, from_output]),
        Err(FxError::FromOutput)
    );
}

#[test]
fn loops_and_duplicates_are_refused() {
    let output = node(2, EffectKind::Output, vec![Some(3)]);
    let a = node(3, EffectKind::Blur, vec![Some(4)]);
    let b = node(4, EffectKind::Glow, vec![Some(3)]);
    assert_eq!(
        EffectGraph::from_nodes(vec![layer(), output.clone(), a.clone(), b]),
        Err(FxError::Cycle)
    );
    let itself = node(3, EffectKind::Blur, vec![Some(3)]);
    assert_eq!(
        EffectGraph::from_nodes(vec![layer(), output.clone(), itself]),
        Err(FxError::Cycle)
    );
    assert_eq!(
        EffectGraph::from_nodes(vec![layer(), output, a.clone(), a]),
        Err(FxError::DuplicateNode(3))
    );
}

#[test]
fn a_loop_away_from_the_output_is_still_refused() {
    let output = node(2, EffectKind::Output, vec![Some(1)]);
    let a = node(3, EffectKind::Blur, vec![Some(4)]);
    let b = node(4, EffectKind::Glow, vec![Some(3)]);
    assert_eq!(
        EffectGraph::from_nodes(vec![layer(), output, a, b]),
        Err(FxError::Cycle)
    );
}

#[test]
fn settings_out_of_range_or_of_the_wrong_kind_are_refused() {
    let output = node(2, EffectKind::Output, vec![Some(3)]);
    let mut values = Node::new(id(3), EffectKind::Blur, [0.0; 2])
        .values()
        .to_vec();
    values[0] = Value::Number(1e9);
    let wild = Node::from_parts(id(3), EffectKind::Blur, vec![Some(id(1))], values, [0.0; 2]);
    assert!(matches!(
        EffectGraph::from_nodes(vec![layer(), output.clone(), wild]),
        Err(FxError::OutOfRange { name: "radius", .. })
    ));
    let colored = Node::from_parts(
        id(3),
        EffectKind::Blur,
        vec![Some(id(1))],
        vec![Value::Color(Rgba8::WHITE)],
        [0.0; 2],
    );
    assert!(EffectGraph::from_nodes(vec![layer(), output.clone(), colored]).is_err());
    let missing = Node::from_parts(id(3), EffectKind::Blur, vec![Some(id(1))], vec![], [0.0; 2]);
    assert_eq!(
        EffectGraph::from_nodes(vec![layer(), output.clone(), missing]),
        Err(FxError::WrongValues(3))
    );
    let lost = Node::from_parts(
        id(3),
        EffectKind::Blur,
        vec![Some(id(1))],
        Node::new(id(3), EffectKind::Blur, [0.0; 2])
            .values()
            .to_vec(),
        [f32::NAN, 0.0],
    );
    assert_eq!(
        EffectGraph::from_nodes(vec![layer(), output, lost]),
        Err(FxError::BadPosition)
    );
}

#[test]
fn a_graph_holds_at_most_the_limit() {
    let mut graph = EffectGraph::new();
    while graph.nodes().len() < MAX_NODES {
        graph = graph.append(EffectKind::Opacity).expect("room").0;
    }
    assert_eq!(graph.append(EffectKind::Blur), Err(FxError::TooManyNodes));
    let mut nodes = graph.nodes().to_vec();
    nodes.push(node(99, EffectKind::Blur, vec![None]));
    assert_eq!(EffectGraph::from_nodes(nodes), Err(FxError::TooManyNodes));
}

#[test]
fn what_the_output_does_not_reach_does_not_count() {
    let (graph, blur) = EffectGraph::new()
        .add(EffectKind::Snow, [0.0, 0.0])
        .expect("add");
    assert!(graph.is_identity());
    assert!(!graph.is_animated(), "the snow reaches nothing");
    assert_eq!(graph.order().len(), 2);
    let joined = graph.connect(NodeId::LAYER, blur, 0).expect("in");
    let joined = joined.connect(blur, NodeId::OUTPUT, 0).expect("out");
    assert!(joined.is_animated());
    assert!(!joined.is_identity());
}

#[test]
fn the_fingerprint_follows_the_picture_and_not_the_view() {
    let (graph, shadow) = EffectGraph::new()
        .append(EffectKind::DropShadow)
        .expect("add");
    let moved = graph.move_node(shadow, [500.0, 40.0]).expect("move");
    assert_eq!(graph.fingerprint(), moved.fingerprint());
    assert!(graph.same_picture(&moved));
    assert_ne!(graph, moved);
    let darker = graph
        .set_value(shadow, "opacity", Value::Number(90.0))
        .expect("set");
    assert_ne!(graph.fingerprint(), darker.fingerprint());
    assert!(!graph.same_picture(&darker));
}

#[test]
fn reach_adds_up_what_spreads() {
    let (graph, shadow) = EffectGraph::new()
        .append(EffectKind::DropShadow)
        .expect("add");
    let graph = graph
        .set_fields(
            shadow,
            &[
                ("distance".into(), "10".into()),
                ("blur".into(), "4".into()),
            ],
        )
        .expect("set");
    assert_eq!(graph.reach(), 10.0 + 6.0 + 1.0);
    let (graph, _) = graph.append(EffectKind::Tint).expect("tint");
    assert_eq!(graph.reach(), 17.0, "a tint spreads nothing");
}

#[test]
fn kinds_are_found_by_name_in_any_spelling() {
    assert_eq!(
        EffectKind::parse("Drop Shadow"),
        Some(EffectKind::DropShadow)
    );
    assert_eq!(
        EffectKind::parse("drop_shadow"),
        Some(EffectKind::DropShadow)
    );
    assert_eq!(EffectKind::parse("SNOW"), Some(EffectKind::Snow));
    assert_eq!(EffectKind::parse("sparkle"), None);
    for kind in EffectKind::ALL {
        assert_eq!(EffectKind::parse(kind.name()), Some(kind));
    }
}
