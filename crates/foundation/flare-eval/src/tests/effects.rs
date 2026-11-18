//! Which shapes a scene layer's effects run over, and how that keys the picture cache.

use flare_fx::{EffectGraph, EffectKind};

use super::*;

fn square(x: f64) -> Content {
    Content::Shape(
        flare_document::Shape::rect(flare_core::ShapeId::new(), x, 0.0, 10.0, 10.0, Rgba8::WHITE)
            .expect("rect"),
    )
}

fn layer(content: Content) -> Layer {
    let span = FrameSpan::new(0, NonZeroU32::new(10).expect("ten"), content);
    Layer::new(LayerId::new(), "L", vec![span]).expect("layer")
}

fn graph(kind: EffectKind) -> EffectGraph {
    EffectGraph::new().append(kind).expect("add").0
}

#[test]
fn a_layer_with_effects_marks_the_shapes_it_drew() {
    // Top to bottom: plain, shadowed, plain. Drawn bottom first, so the shadowed one is shape 1.
    let shadowed = layer(square(20.0)).with_effects(Some(graph(EffectKind::DropShadow)));
    let id = shadowed.id();
    let (doc, scene) = document(
        Timeline::new(vec![layer(square(40.0)), shadowed, layer(square(0.0))]),
        vec![],
    );
    let frame = evaluate_frame(&doc, scene, 3).expect("frame");
    assert_eq!(frame.shapes.len(), 3);
    assert_eq!(frame.effects.len(), 1);
    let run = &frame.effects[0];
    assert_eq!(run.layer, id);
    assert_eq!(run.shapes, 1..2);
    assert_eq!(run.bitmaps, 0..0);
    assert_eq!(run.frame, 3);
    assert_eq!(run.stage, [1920.0, 1080.0], "the default stage");
}

#[test]
fn a_layer_that_draws_nothing_still_runs_its_snow_and_a_hidden_one_does_not() {
    let snowing = layer(Content::Empty).with_effects(Some(graph(EffectKind::Snow)));
    let hidden = layer(square(0.0))
        .with_effects(Some(graph(EffectKind::Blur)))
        .with_visible(false);
    let (doc, scene) = document(Timeline::new(vec![snowing, hidden]), vec![]);
    let frame = evaluate_frame(&doc, scene, 0).expect("frame");
    assert_eq!(frame.effects.len(), 1);
    assert_eq!(frame.effects[0].shapes, 0..0);
}

#[test]
fn a_graph_that_draws_the_layer_as_it_is_runs_nothing() {
    let (unconnected, _) = EffectGraph::new()
        .add(EffectKind::Blur, [0.0, 0.0])
        .expect("add");
    let plain = layer(square(0.0)).with_effects(Some(unconnected));
    assert!(plain.effects().is_some(), "kept while it is built");
    let (doc, scene) = document(Timeline::new(vec![plain]), vec![]);
    assert!(
        evaluate_frame(&doc, scene, 0)
            .expect("frame")
            .effects
            .is_empty()
    );
}

#[test]
fn past_the_limit_layers_are_drawn_without_their_effects_and_say_so() {
    let layers = (0..MAX_EFFECT_RUNS + 2)
        .map(|i| layer(square(i as f64)).with_effects(Some(graph(EffectKind::Tint))))
        .collect();
    let (doc, scene) = document(Timeline::new(layers), vec![]);
    let frame = evaluate_frame(&doc, scene, 0).expect("frame");
    assert_eq!(frame.effects.len(), MAX_EFFECT_RUNS);
    assert_eq!(frame.warnings.len(), 2);
    assert_eq!(frame.shapes.len(), MAX_EFFECT_RUNS + 2, "still drawn");
}

#[test]
fn moving_effects_give_each_frame_its_own_picture_and_still_ones_do_not() {
    let picture = |kind| {
        let (doc, scene) = document(
            Timeline::new(vec![layer(square(0.0)).with_effects(Some(graph(kind)))]),
            vec![],
        );
        let at = |f| evaluate_frame(&doc, scene, f).expect("frame").signature();
        (at(1), at(2))
    };
    let (a, b) = picture(EffectKind::Snow);
    assert_ne!(a, b, "snow falls on a held drawing");
    let (a, b) = picture(EffectKind::Glow);
    assert_eq!(a, b, "a glow on a held drawing is one picture");
    let (doc, scene) = document(Timeline::new(vec![layer(square(0.0))]), vec![]);
    let plain = evaluate_frame(&doc, scene, 1).expect("frame").signature();
    assert_ne!(plain, b, "effects change the picture");
}
