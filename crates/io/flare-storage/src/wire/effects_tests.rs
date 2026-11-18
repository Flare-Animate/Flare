#![allow(clippy::expect_used)]

use flare_fx::{EffectGraph, EffectKind, NodeId, Value};

use super::*;
use crate::codec::{from_cbor, to_cbor};

fn graph() -> EffectGraph {
    let (graph, shadow) = EffectGraph::new()
        .append(EffectKind::DropShadow)
        .expect("add");
    let (graph, snow) = graph.append(EffectKind::Snow).expect("add");
    graph
        .set_value(shadow, "color", Value::Color(Rgba8::new(10, 20, 30, 200)))
        .and_then(|g| g.set_value(snow, "flakes", Value::Number(42.0)))
        .and_then(|g| g.move_node(snow, [33.5, -12.0]))
        .expect("set")
}

#[test]
fn a_graph_comes_back_exactly_through_cbor() {
    let bytes = to_cbor(&EffectsWire::of(&graph())).expect("encode");
    let back: EffectsWire = from_cbor(&bytes).expect("decode");
    assert_eq!(back.build().expect("build"), graph());
}

#[test]
fn a_setting_the_file_lacks_takes_its_default() {
    let mut wire = EffectsWire::of(&graph());
    for node in &mut wire.nodes {
        node.params.remove("opacity");
    }
    let built = wire.build().expect("build");
    let shadow = built
        .nodes()
        .iter()
        .find(|n| n.kind() == EffectKind::DropShadow)
        .expect("shadow");
    assert_eq!(shadow.value("opacity"), Some(Value::Number(60.0)));
}

#[test]
fn a_damaged_graph_is_refused_by_name() {
    let refused = |edit: &dyn Fn(&mut EffectsWire)| {
        let mut wire = EffectsWire::of(&graph());
        edit(&mut wire);
        match wire.build() {
            Err(StorageError::Corrupt(message)) => message,
            other => panic!(
                "expected a refusal, got {:?}",
                other.map(|g| g.nodes().len())
            ),
        }
    };
    let loop_back = refused(&|w| w.nodes[2].inputs = vec![Some(NodeId::OUTPUT.get() + 2)]);
    assert!(loop_back.contains("loop"), "{loop_back}");
    let unknown = refused(&|w| w.nodes[2].kind = "sparkle".into());
    assert!(unknown.contains("sparkle"), "{unknown}");
    let setting = refused(&|w| {
        w.nodes[2]
            .params
            .insert("glitter".into(), ValueWire::Number(1.0));
    });
    assert!(setting.contains("glitter"), "{setting}");
    let range = refused(&|w| {
        w.nodes[2]
            .params
            .insert("distance".into(), ValueWire::Number(-5.0));
    });
    assert!(range.contains("distance"), "{range}");
    let zero = refused(&|w| w.nodes[2].id = 0);
    assert!(zero.contains("0"), "{zero}");
    let missing = refused(&|w| {
        w.nodes.remove(1);
    });
    assert!(missing.contains("Output"), "{missing}");
}
