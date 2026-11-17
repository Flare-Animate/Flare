//! Running whole graphs: chains, a node read twice, nothing connected, a zoomed picture.

use flare_core::Rgba8;

use super::{square, total_alpha, whole};
use crate::{EffectGraph, EffectKind, FxError, NodeId, Pixels, Space, Value, run};

#[test]
fn the_plain_graph_gives_back_the_layer() {
    let p = square(20, 20, [2, 2, 9, 9], [10, 20, 30, 255]);
    let out = run(&EffectGraph::new(), p.clone(), &whole(20, 20), 0).expect("run");
    assert_eq!(out, p);
}

#[test]
fn a_disconnected_output_draws_nothing() {
    let graph = EffectGraph::new()
        .disconnect(NodeId::OUTPUT, 0)
        .expect("cut");
    let p = square(8, 8, [0, 0, 8, 8], [255, 255, 255, 255]);
    let out = run(&graph, p, &whole(8, 8), 0).expect("run");
    assert_eq!(total_alpha(&out), 0);
}

#[test]
fn a_chain_runs_in_order() {
    // A tint after a shadow colors the shadow too; the other way round it would stay black.
    let (graph, shadow) = EffectGraph::new()
        .append(EffectKind::DropShadow)
        .expect("add");
    let graph = graph
        .set_fields(
            shadow,
            &[
                ("distance".into(), "6".into()),
                ("angle".into(), "0".into()),
                ("blur".into(), "0".into()),
                ("opacity".into(), "100".into()),
            ],
        )
        .expect("set");
    let (graph, tint) = graph.append(EffectKind::Tint).expect("add");
    let graph = graph
        .set_value(tint, "amount", Value::Number(100.0))
        .and_then(|g| g.set_value(tint, "color", Value::Color(Rgba8::opaque(0, 255, 0))))
        .expect("set");
    let p = square(20, 4, [0, 0, 4, 4], [255, 0, 0, 255]);
    let out = run(&graph, p, &whole(20, 4), 0).expect("run");
    assert_eq!(out.pixel(1, 1), [0, 255, 0, 255]);
    assert_eq!(out.pixel(8, 1), [0, 255, 0, 255], "the shadow was tinted");
}

#[test]
fn a_node_read_twice_feeds_both_readers() {
    // The layer goes to both inputs of Over: the picture over itself.
    let (graph, over) = EffectGraph::new().append(EffectKind::Over).expect("add");
    let graph = graph.connect(NodeId::LAYER, over, 0).expect("both");
    let p = square(4, 4, [0, 0, 2, 2], [128, 0, 0, 128]);
    let out = run(&graph, p, &whole(4, 4), 0).expect("run");
    // Half over half: 128 + 128 * (127 / 255), rounded.
    assert_eq!(out.pixel(0, 0), [192, 0, 0, 192]);
}

#[test]
fn an_unconnected_input_is_transparent() {
    let (graph, over) = EffectGraph::new().append(EffectKind::Over).expect("add");
    assert_eq!(graph.node(over).map(|n| n.inputs()[0]), Some(None));
    let p = square(4, 4, [0, 0, 4, 4], [9, 9, 9, 255]);
    let out = run(&graph, p.clone(), &whole(4, 4), 0).expect("run");
    assert_eq!(out, p);
}

#[test]
fn lengths_are_measured_on_the_stage_whatever_the_zoom() {
    let (graph, shadow) = EffectGraph::new()
        .append(EffectKind::DropShadow)
        .expect("add");
    let graph = graph
        .set_fields(
            shadow,
            &[
                ("distance".into(), "5".into()),
                ("angle".into(), "90".into()),
                ("blur".into(), "0".into()),
                ("opacity".into(), "100".into()),
            ],
        )
        .expect("set");
    // Twice the zoom: the stage's 4x4 square is 8x8 pixels and the shadow is 10 pixels down.
    let p = square(16, 40, [0, 0, 8, 8], [255, 255, 255, 255]);
    let zoomed = Space {
        scale: 2.0,
        origin: [0.0, 0.0],
        stage: [8.0, 20.0],
    };
    let out = run(&graph, p, &zoomed, 0).expect("run");
    assert_eq!(out.pixel(4, 15), [0, 0, 0, 255]);
    assert_eq!(out.pixel(4, 19), [0, 0, 0, 0]);
}

#[test]
fn snow_through_the_graph_follows_the_frame() {
    let (graph, _) = EffectGraph::new().append(EffectKind::Snow).expect("add");
    let empty = || Pixels::transparent(64, 48).expect("pixels");
    let a = run(&graph, empty(), &whole(64, 48), 4).expect("run");
    let b = run(&graph, empty(), &whole(64, 48), 4).expect("run");
    let c = run(&graph, empty(), &whole(64, 48), 5).expect("run");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert!(total_alpha(&a) > 0);
}

#[test]
fn a_bad_scale_or_picture_is_refused() {
    let p = Pixels::transparent(2, 2).expect("pixels");
    let bad = Space {
        scale: 0.0,
        ..whole(2, 2)
    };
    assert_eq!(run(&EffectGraph::new(), p, &bad, 0), Err(FxError::BadScale));
    assert_eq!(Pixels::new(2, 2, vec![0; 3]), Err(FxError::BadPixels));
    assert_eq!(
        Pixels::new(u32::MAX, u32::MAX, vec![]),
        Err(FxError::BadPixels)
    );
    // A blank picture too large to count is refused, not made with too few pixels.
    assert_eq!(
        Pixels::transparent(u32::MAX, u32::MAX),
        Err(FxError::BadPixels)
    );
}

#[test]
fn a_cleared_picture_is_the_same_size_and_empty() {
    let p = square(7, 5, [1, 1, 4, 4], [10, 20, 30, 255]);
    let blank = p.cleared();
    assert_eq!((blank.width(), blank.height()), (7, 5));
    assert_eq!(blank.data().len(), p.data().len());
    assert_eq!(total_alpha(&blank), 0);
    assert_eq!(blank, Pixels::transparent(7, 5).expect("pixels"));
}
