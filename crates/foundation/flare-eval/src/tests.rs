#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Transform2D};
use flare_document::{FrameSpan, Layer, Scene, Symbol};
use glamx::DVec2;

use super::*;

fn at(x: f64) -> Transform2D {
    Transform2D::new(DAffine2::from_translation(DVec2::new(x, 0.0))).expect("finite")
}

fn span(start: u32, duration: u32, symbol: SymbolId, transform: Transform2D) -> FrameSpan {
    let instance = Instance::new(InstanceId::new(), symbol, transform);
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Instance(instance),
    )
}

fn timeline(layers: Vec<Vec<FrameSpan>>) -> Timeline {
    Timeline::new(
        layers
            .into_iter()
            .map(|s| Layer::new(LayerId::new(), "L", s).expect("layer"))
            .collect(),
    )
}

fn document(scene: Timeline, symbols: Vec<Symbol>) -> (Document, SceneId) {
    let id = SceneId::new();
    let rate = FrameRate::new(24, 1).expect("rate");
    let scenes = vec![Scene::new(id, "S", scene)];
    (
        Document::new(DocumentId::new(), rate, scenes, symbols).expect("document"),
        id,
    )
}

#[test]
fn two_instances_of_one_symbol_keep_separate_paths_and_transforms() {
    let art = SymbolId::new();
    let scene = timeline(vec![
        vec![span(0, 10, art, at(100.0))],
        vec![span(0, 10, art, at(5.0))],
    ]);
    let (doc, id) = document(scene, vec![Symbol::new(art, "Art", Timeline::default())]);
    let placed = evaluate(&doc, id, 3).expect("eval");
    assert_eq!(placed.len(), 2);
    assert_ne!(placed[0].path, placed[1].path);
    // Bottom layer first.
    assert_eq!(placed[0].transform.translation.x, 5.0);
    assert_eq!(placed[1].transform.translation.x, 100.0);
}

#[test]
fn nested_symbols_compose_transforms_and_loop() {
    let (leaf, inner, outer) = (SymbolId::new(), SymbolId::new(), SymbolId::new());
    // Inner is a 4-frame loop: leaf at x=1 for two frames, then x=2 for two.
    let inner_tl = timeline(vec![vec![
        span(0, 2, leaf, at(1.0)),
        span(2, 2, leaf, at(2.0)),
    ]]);
    let outer_tl = timeline(vec![vec![span(0, 8, inner, at(10.0))]]);
    let symbols = vec![
        Symbol::new(leaf, "Leaf", Timeline::default()),
        Symbol::new(inner, "Inner", inner_tl),
        Symbol::new(outer, "Outer", outer_tl),
    ];
    let scene = timeline(vec![vec![span(5, 20, outer, at(100.0))]]);
    let (doc, id) = document(scene, symbols);

    let leaf_x = |frame| {
        let placed = evaluate(&doc, id, frame).expect("eval");
        assert_eq!(placed.len(), 3);
        assert_eq!(placed[2].path.len(), 3);
        placed[2].transform.translation.x
    };
    assert_eq!(leaf_x(5), 111.0); // inner frame 0
    assert_eq!(leaf_x(8), 112.0); // inner frame 3
    assert_eq!(leaf_x(10), 111.0); // inner frame 5 loops to 1
}

#[test]
fn frames_outside_spans_and_empty_spans_place_nothing() {
    let art = SymbolId::new();
    let scene = timeline(vec![vec![
        span(2, 2, art, at(0.0)),
        FrameSpan::new(4, NonZeroU32::new(2).expect("nonzero"), Content::Empty),
    ]]);
    let (doc, id) = document(scene, vec![Symbol::new(art, "Art", Timeline::default())]);
    assert!(evaluate(&doc, id, 0).expect("eval").is_empty());
    assert_eq!(evaluate(&doc, id, 3).expect("eval").len(), 1);
    assert!(evaluate(&doc, id, 4).expect("eval").is_empty());
    assert!(evaluate(&doc, id, 99).expect("eval").is_empty());
}

fn tween_scene(from: Transform2D, to: Transform2D) -> (Document, SceneId) {
    let art = SymbolId::new();
    let first = span(0, 10, art, from).with_tween(Tween::Linear);
    let scene = timeline(vec![vec![first, span(10, 5, art, to)]]);
    document(scene, vec![Symbol::new(art, "Art", Timeline::default())])
}

fn rotation(angle: f64) -> Transform2D {
    Transform2D::new(DAffine2::from_angle(angle)).expect("finite")
}

#[test]
fn linear_tween_interpolates_translation_and_holds_at_the_end() {
    let (doc, id) = tween_scene(at(0.0), at(100.0));
    let x = |frame| {
        evaluate(&doc, id, frame).expect("eval")[0]
            .transform
            .translation
            .x
    };
    assert_eq!(x(0), 0.0);
    assert_eq!(x(5), 50.0);
    assert_eq!(x(10), 100.0);
    assert_eq!(x(12), 100.0, "last span has no tween and holds");
}

#[test]
fn rotation_tween_takes_the_shortest_arc() {
    use std::f64::consts::PI;
    let (doc, id) = tween_scene(rotation(0.9 * PI), rotation(-0.9 * PI));
    let mid = evaluate(&doc, id, 5).expect("eval")[0].transform;
    // Halfway along the short arc is pi, not 0.
    let (_, angle, _) = mid.to_scale_angle_translation();
    assert!((angle.abs() - PI).abs() < 1e-9, "angle {angle}");
}

#[test]
fn tweening_a_sheared_matrix_is_an_error_not_an_approximation() {
    let shear = Transform2D::new(DAffine2::from_cols_array(&[1.0, 0.0, 0.5, 1.0, 0.0, 0.0]))
        .expect("finite");
    let (doc, id) = tween_scene(shear, at(0.0));
    assert!(matches!(
        evaluate(&doc, id, 3),
        Err(EvalError::UnsupportedTween(_))
    ));
    // Frames of a span without a tween never decompose.
    assert!(evaluate(&doc, id, 11).is_ok());
}

#[test]
fn tween_needs_an_adjacent_span_with_the_same_symbol() {
    let (a, b) = (SymbolId::new(), SymbolId::new());
    let gap = timeline(vec![vec![
        span(0, 5, a, at(0.0)).with_tween(Tween::Linear),
        span(6, 5, a, at(100.0)),
    ]]);
    let other = timeline(vec![vec![
        span(0, 5, a, at(0.0)).with_tween(Tween::Linear),
        span(5, 5, b, at(100.0)),
    ]]);
    for scene in [gap, other] {
        let symbols = vec![
            Symbol::new(a, "A", Timeline::default()),
            Symbol::new(b, "B", Timeline::default()),
        ];
        let (doc, id) = document(scene, symbols);
        assert_eq!(
            evaluate(&doc, id, 2).expect("eval")[0]
                .transform
                .translation
                .x,
            0.0
        );
    }
}

#[test]
fn shapes_are_emitted_in_paint_order_with_composed_transforms() {
    use flare_core::{Rgba8, ShapeId};
    let art = SymbolId::new();
    let shape =
        |x| Shape::rect(ShapeId::new(), x, 0.0, 1.0, 1.0, Rgba8::opaque(1, 2, 3)).expect("rect");
    let art_tl = timeline(vec![vec![FrameSpan::new(
        0,
        NonZeroU32::new(1).expect("nonzero"),
        Content::Shape(shape(7.0)),
    )]]);
    let scene_shape = FrameSpan::new(
        0,
        NonZeroU32::new(10).expect("nonzero"),
        Content::Shape(shape(0.0)),
    );
    // Layers are top to bottom: the shape layer is on top, the instance layer below it.
    let scene = timeline(vec![vec![scene_shape], vec![span(0, 10, art, at(100.0))]]);
    let (doc, id) = document(scene, vec![Symbol::new(art, "Art", art_tl)]);
    let frame = evaluate_frame(&doc, id, 2).expect("eval");
    assert_eq!(frame.shapes.len(), 2);
    // Bottom layer first: the symbol's shape, offset by the instance, then the top shape.
    assert_eq!(frame.shapes[0].transform.translation.x, 100.0);
    assert_eq!(frame.shapes[0].path.len(), 1);
    assert!(frame.shapes[1].path.is_empty());
    assert_eq!(frame.placed.len(), 1);
}

#[test]
fn hidden_layers_place_and_draw_nothing_including_inside_symbols() {
    use flare_core::{Rgba8, ShapeId};
    let art = SymbolId::new();
    let shape = |x| {
        Content::Shape(Shape::rect(ShapeId::new(), x, 0.0, 1.0, 1.0, Rgba8::WHITE).expect("rect"))
    };
    let one = |content| FrameSpan::new(0, NonZeroU32::new(5).expect("nonzero"), content);
    let hidden_inside = Timeline::new(vec![
        Layer::new(LayerId::new(), "shown", vec![one(shape(1.0))]).expect("layer"),
        Layer::new(LayerId::new(), "hidden", vec![one(shape(2.0))])
            .expect("layer")
            .with_visible(false),
    ]);
    let scene = Timeline::new(vec![
        Layer::new(LayerId::new(), "hidden", vec![span(0, 5, art, at(0.0))])
            .expect("layer")
            .with_visible(false),
        Layer::new(LayerId::new(), "shown", vec![span(0, 5, art, at(10.0))]).expect("layer"),
    ]);
    let (doc, id) = document(scene, vec![Symbol::new(art, "Art", hidden_inside)]);
    let frame = evaluate_frame(&doc, id, 1).expect("eval");
    assert_eq!(
        frame.placed.len(),
        1,
        "the hidden scene layer's instance is skipped"
    );
    assert_eq!(
        frame.shapes.len(),
        1,
        "the hidden layer inside the symbol is skipped too"
    );
    assert_eq!(frame.placed[0].transform.translation.x, 10.0);
}

#[test]
fn unknown_scene_is_an_error() {
    let (doc, _) = document(Timeline::default(), vec![]);
    let ghost = SceneId::new();
    assert_eq!(evaluate(&doc, ghost, 0), Err(EvalError::NoSuchScene(ghost)));
}

#[test]
fn excessive_nesting_is_bounded() {
    let ids: Vec<SymbolId> = (0..=MAX_DEPTH + 1).map(|_| SymbolId::new()).collect();
    let mut symbols: Vec<Symbol> = ids
        .windows(2)
        .map(|pair| {
            Symbol::new(
                pair[0],
                "S",
                timeline(vec![vec![span(0, 1, pair[1], at(0.0))]]),
            )
        })
        .collect();
    symbols.push(Symbol::new(ids[ids.len() - 1], "leaf", Timeline::default()));
    let scene = timeline(vec![vec![span(0, 1, ids[0], at(0.0))]]);
    let (doc, id) = document(scene, symbols);
    assert_eq!(evaluate(&doc, id, 0), Err(EvalError::TooDeep));
}

#[test]
fn a_held_exposure_has_one_signature_and_any_visible_change_has_another() {
    let art = SymbolId::new();
    let moving = timeline(vec![vec![
        span(0, 4, art, at(10.0)),
        span(4, 4, art, at(20.0)),
    ]]);
    // The symbol holds a rectangle, so moving the instance moves something visible.
    let rect = flare_document::Shape::rect(
        flare_core::ShapeId::new(),
        0.0,
        0.0,
        5.0,
        5.0,
        flare_core::Rgba8::WHITE,
    )
    .expect("rect");
    let inner = Timeline::new(vec![
        Layer::new(
            LayerId::new(),
            "inner",
            vec![FrameSpan::new(
                0,
                NonZeroU32::new(1).expect("nonzero"),
                Content::Shape(rect),
            )],
        )
        .expect("layer"),
    ]);
    let (doc, id) = document(moving, vec![Symbol::new(art, "Art", inner)]);
    let sig = |f| evaluate_frame(&doc, id, f).expect("eval").signature();
    // Frames 0..4 show the same thing; frame 4 starts the next exposure.
    assert_eq!(sig(0), sig(3));
    assert_ne!(sig(3), sig(4));

    let rect_layer = |name: &str| {
        let shape = flare_document::Shape::rect(
            flare_core::ShapeId::new(),
            0.0,
            0.0,
            10.0,
            10.0,
            flare_core::Rgba8::WHITE,
        )
        .expect("rect");
        let held = FrameSpan::new(
            0,
            NonZeroU32::new(5).expect("nonzero"),
            Content::Shape(shape),
        );
        Layer::new(LayerId::new(), name, vec![held]).expect("layer")
    };
    let layer = rect_layer("a");
    let (one, one_id) = document(Timeline::new(vec![layer.clone()]), vec![]);
    let (two, two_id) = document(Timeline::new(vec![layer, rect_layer("b")]), vec![]);
    let s1 = evaluate_frame(&one, one_id, 0).expect("eval").signature();
    let s2 = evaluate_frame(&two, two_id, 0).expect("eval").signature();
    assert_ne!(s1, s2, "an extra shape changes it");
    assert_eq!(
        s1,
        evaluate_frame(&one, one_id, 0).expect("eval").signature(),
        "and the same frame always hashes the same"
    );
}

fn shape_layer(name: &str, x: f64, w: f64, mask: bool, visible: bool) -> Layer {
    let shape = flare_document::Shape::rect(
        flare_core::ShapeId::new(),
        x,
        0.0,
        w,
        10.0,
        flare_core::Rgba8::WHITE,
    )
    .expect("rect");
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(5).expect("nonzero"),
        Content::Shape(shape),
    );
    Layer::new(LayerId::new(), name, vec![span])
        .expect("layer")
        .with_mask(mask)
        .with_visible(visible)
}

fn masked_scene(layers: Vec<Layer>) -> (Document, SceneId) {
    document(Timeline::new(layers), vec![])
}

mod more;
