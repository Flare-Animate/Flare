//! A layer's effects in a render: in the layer's place in the paint order, measured on the stage,
//! taking in what lies just outside a zoomed view, and on the stage's own path too.

use flare_fx::{EffectGraph, EffectKind, Value};

use super::*;

const WHITE: Rgba8 = Rgba8::WHITE;
const BLACK: Rgba8 = Rgba8::opaque(0, 0, 0);

/// A hard black shadow `distance` to the right.
fn hard_shadow(distance: f32) -> EffectGraph {
    let (graph, shadow) = EffectGraph::new()
        .append(EffectKind::DropShadow)
        .expect("add");
    [
        ("distance", distance),
        ("angle", 0.0),
        ("blur", 0.0),
        ("opacity", 100.0),
    ]
    .into_iter()
    .try_fold(graph, |g, (name, v)| {
        g.set_value(shadow, name, Value::Number(v))
    })
    .expect("set")
}

fn layer(content: Content) -> Layer {
    Layer::new(LayerId::new(), "L", vec![span(content)]).expect("layer")
}

#[test]
fn a_layer_whose_effects_change_nothing_draws_exactly_as_without() {
    // Full opacity is the picture as it came in: what comes back must be laid pixel for pixel.
    let (graph, _) = EffectGraph::new().append(EffectKind::Opacity).expect("add");
    let graph = graph
        .set_value(
            flare_fx::NodeId::new(3).expect("id"),
            "opacity",
            Value::Number(100.0),
        )
        .expect("set");
    let art = || layer(rect(3.3, 2.0, 9.4, 11.7, RED));
    let (plain, id) = scene_with_layers(vec![art()]);
    let (effected, id2) = scene_with_layers(vec![art().with_effects(Some(graph))]);
    assert_eq!(
        draw(&plain, id, WHITE).rgba,
        draw(&effected, id2, WHITE).rgba
    );
}

#[test]
fn a_drop_shadow_falls_beside_the_shape_and_under_the_layers_above() {
    let shadowed = layer(rect(2.0, 2.0, 6.0, 6.0, RED)).with_effects(Some(hard_shadow(8.0)));
    let cover = layer(rect(14.0, 0.0, 6.0, 20.0, WHITE));
    let (doc, id) = scene_with_layers(vec![cover, shadowed]);
    let image = draw(&doc, id, Rgba8::opaque(0, 255, 0));
    assert_eq!(
        image.pixel(4, 4),
        Some(RED),
        "the shape on top of its shadow"
    );
    assert_eq!(
        image.pixel(11, 4),
        Some(BLACK),
        "the shadow, eight to the right"
    );
    assert_eq!(
        image.pixel(15, 4),
        Some(WHITE),
        "the layer above covers the shadow"
    );
    assert_eq!(
        image.pixel(4, 12),
        Some(Rgba8::opaque(0, 255, 0)),
        "the ground elsewhere"
    );
}

#[test]
fn a_shadow_from_outside_a_zoomed_view_still_falls_inside_it() {
    // The shape is left of the view; its shadow reaches into it. At twice the zoom.
    let shadowed = layer(rect(0.0, 0.0, 10.0, 10.0, RED)).with_effects(Some(hard_shadow(10.0)));
    let (doc, id) = scene_with_layers(vec![shadowed]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    let view = Viewport {
        x: 12.0,
        y: 0.0,
        width: 10.0,
        height: 10.0,
    };
    let image = render_viewport(&frame, &view, 20, 20, WHITE).expect("render");
    assert_eq!(
        image.pixel(10, 10),
        Some(BLACK),
        "stage x 17 is in the shadow"
    );
    assert_eq!(image.pixel(18, 10), Some(WHITE), "stage x 21 is past it");
}

#[test]
fn the_stage_with_a_transparent_layer_still_runs_effects() {
    let shadowed = layer(rect(2.0, 2.0, 6.0, 6.0, RED)).with_effects(Some(hard_shadow(8.0)));
    let rough = layer(rect(0.0, 16.0, 20.0, 4.0, RED)).with_view_opacity(50);
    let (doc, id) = scene_with_layers(vec![rough, shadowed]);
    let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    assert!(has_view_alpha(&frame));
    let image =
        render_stage_viewport(&frame, &Viewport::full(20, 20), 20, 20, WHITE).expect("stage");
    assert_eq!(image.pixel(11, 4), Some(BLACK));
}

#[test]
fn snow_on_an_empty_layer_falls_over_the_stage_and_moves() {
    let (graph, _) = EffectGraph::new().append(EffectKind::Snow).expect("add");
    let sky = Layer::new(
        LayerId::new(),
        "Snow",
        vec![FrameSpan::new(
            0,
            NonZeroU32::new(10).expect("ten"),
            Content::Empty,
        )],
    )
    .expect("layer")
    .with_effects(Some(graph));
    let (doc, id) = scene_with_layers(vec![sky]);
    let at = |f| {
        let frame = flare_eval::evaluate_frame(&doc, id, f).expect("eval");
        render(&frame, 192, 108, BLACK).expect("render")
    };
    let (a, b) = (at(2), at(3));
    assert_ne!(a.rgba, b.rgba, "it falls");
    assert!(
        a.rgba.as_chunks::<4>().0.iter().any(|p| p[0] > 100),
        "white flakes on black"
    );
    assert_eq!(a.rgba, at(2).rgba, "the same frame, the same snow");
}

#[test]
fn runs_out_of_order_in_a_hand_made_frame_are_drawn_plainly_once() {
    let shadowed = layer(rect(2.0, 2.0, 6.0, 6.0, RED)).with_effects(Some(hard_shadow(8.0)));
    let (doc, id) = scene_with_layers(vec![shadowed]);
    let mut frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
    let mut bad = frame.effects[0].clone();
    bad.shapes = 0..5;
    frame.effects = vec![bad];
    let image = render(&frame, 20, 20, WHITE).expect("render");
    assert_eq!(image.pixel(4, 4), Some(RED));
    assert_eq!(
        image.pixel(11, 4),
        Some(WHITE),
        "no shadow from a run past the end"
    );
}
