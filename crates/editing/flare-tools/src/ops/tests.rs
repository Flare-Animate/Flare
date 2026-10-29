#![allow(clippy::expect_used)]

use super::*;
use emath::{Pos2, Rect};
use flare_commands::Command;
use flare_core::{
    DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId, Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, Scene, Shape, Symbol, Timeline, Tween,
};
use flare_history::EditorSession;
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;

const RED: Rgba8 = Rgba8::opaque(255, 0, 0);

struct Fixture {
    session: EditorSession,
    scene: SceneId,
    layer: LayerId,
}

fn fixture() -> Fixture {
    let (scene, layer) = (SceneId::new(), LayerId::new());
    let layer_value = Layer::new(layer, "L", vec![]).expect("layer");
    let scene_value = Scene::new(scene, "S", Timeline::new(vec![layer_value]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let doc = Document::new(DocumentId::new(), rate, vec![scene_value], vec![]).expect("doc");
    Fixture {
        session: EditorSession::new(doc),
        scene,
        layer,
    }
}

fn rect_span(start: u32, duration: u32, x: f64, y: f64, w: f64, h: f64) -> FrameSpan {
    let shape = Shape::rect(ShapeId::new(), x, y, w, h, RED).expect("rect");
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Shape(shape),
    )
}

fn add(f: &mut Fixture, span: FrameSpan) {
    let command = Command::InsertSpan {
        scene: f.scene,
        layer: f.layer,
        span,
    };
    f.session.apply([command]).expect("insert");
}

fn at(f: &Fixture, start: u32) -> SpanRef {
    SpanRef {
        layer: f.layer,
        start,
    }
}

fn bounds(f: &Fixture, start: u32) -> Rect {
    span_bounds(f.session.document(), f.scene, at(f, start)).expect("bounds")
}

#[test]
fn translating_a_shape_moves_it_and_keeps_duration_and_identity() {
    let mut f = fixture();
    add(&mut f, rect_span(3, 10, 0.0, 0.0, 20.0, 10.0));
    let before = f.session.document().clone();
    let commands = translate(&before, f.scene, at(&f, 3), DVec2::new(5.0, -2.0)).expect("move");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, at(&f, 3)).expect("span");
    assert_eq!(span.duration().get(), 10);
    let old = find_span(&before, f.scene, at(&f, 3)).expect("old");
    let (Content::Shape(a), Content::Shape(b)) = (old.content(), span.content()) else {
        panic!("shapes expected");
    };
    assert_eq!(a.id(), b.id());
    assert_eq!(
        bounds(&f, 3),
        Rect::from_min_max(Pos2::new(5.0, -2.0), Pos2::new(25.0, 8.0))
    );
    f.session.undo();
    assert_eq!(f.session.document(), &before);
}

#[test]
fn translating_an_instance_composes_the_translation_and_keeps_tween() {
    let mut f = fixture();
    let symbol = SymbolId::new();
    let rotate =
        Transform2D::new(DAffine2::from_angle(std::f64::consts::FRAC_PI_2)).expect("finite");
    let instance = Instance::new(InstanceId::new(), symbol, rotate);
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(5).expect("nonzero"),
        Content::Instance(instance),
    )
    .with_tween(Tween::Linear);
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            symbol,
            "S",
            Timeline::default(),
        ))])
        .expect("symbol");
    add(&mut f, span);
    let commands = translate(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(10.0, 0.0),
    )
    .expect("move");
    f.session.apply(commands).expect("apply");
    let moved = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    assert_eq!(moved.tween(), Tween::Linear);
    let Content::Instance(instance) = moved.content() else {
        panic!("instance expected")
    };
    let affine = instance.transform().as_affine();
    assert_eq!(affine.translation, DVec2::new(10.0, 0.0));
    assert!(
        (affine.matrix2.x_axis.y - 1.0).abs() < 1e-12,
        "rotation is preserved"
    );
}

#[test]
fn degenerate_translations_do_nothing() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 5, 0.0, 0.0, 5.0, 5.0));
    let doc = f.session.document();
    assert!(translate(doc, f.scene, at(&f, 0), DVec2::ZERO).is_none());
    assert!(translate(doc, f.scene, at(&f, 99), DVec2::ONE).is_none());
    assert!(translate(doc, f.scene, at(&f, 0), DVec2::new(f64::NAN, 0.0)).is_none());
}

#[test]
fn convert_to_symbol_replaces_the_shape_with_an_equivalent_instance() {
    let mut f = fixture();
    add(&mut f, rect_span(2, 8, 30.0, 40.0, 10.0, 10.0));
    let before = flare_eval::evaluate_frame(f.session.document(), f.scene, 4).expect("eval");
    let before_id = before.shapes[0].shape.id();
    let (commands, symbol) =
        convert_to_symbol(f.session.document(), f.scene, at(&f, 2)).expect("convert");
    f.session.apply(commands).expect("apply");

    let doc = f.session.document();
    assert_eq!(doc.symbols().len(), 1);
    assert_eq!(doc.symbols()[0].name(), "Symbol 1");
    let span = find_span(doc, f.scene, at(&f, 2)).expect("span");
    assert_eq!(span.duration().get(), 8);
    assert!(matches!(span.content(), Content::Instance(i) if i.symbol() == symbol));
    // Same artwork on stage: same shape, identity transform, one nesting level.
    let after = flare_eval::evaluate_frame(doc, f.scene, 4).expect("eval");
    assert_eq!(after.shapes.len(), 1);
    assert_eq!(after.shapes[0].shape.id(), before_id);
    assert_eq!(after.shapes[0].transform, DAffine2::IDENTITY);
    assert_eq!(after.shapes[0].path.len(), 1);
    // Undo restores the original in one step.
    f.session.undo();
    assert_eq!(f.session.document().symbols().len(), 0);
}

#[test]
fn only_shapes_can_be_converted() {
    let mut f = fixture();
    let symbol = SymbolId::new();
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            symbol,
            "S",
            Timeline::default(),
        ))])
        .expect("symbol");
    let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
    let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Instance(instance));
    add(&mut f, span);
    assert!(convert_to_symbol(f.session.document(), f.scene, at(&f, 0)).is_none());
    assert!(convert_to_symbol(f.session.document(), f.scene, at(&f, 5)).is_none());
}

#[test]
fn placing_an_instance_centers_the_symbol_on_the_target_point() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 24, 0.0, 0.0, 10.0, 10.0));
    let (commands, symbol) =
        convert_to_symbol(f.session.document(), f.scene, at(&f, 0)).expect("convert");
    f.session.apply(commands).expect("convert");
    let commands = place_instance(
        f.session.document(),
        f.scene,
        symbol,
        DVec2::new(100.0, 60.0),
    )
    .expect("place");
    f.session.apply(commands).expect("place");
    let scene = &f.session.document().scenes()[0];
    let new_layer = scene.timeline().layers().last().expect("layer").id();
    let placed = span_bounds(
        f.session.document(),
        f.scene,
        SpanRef {
            layer: new_layer,
            start: 0,
        },
    )
    .expect("bounds");
    assert_eq!(placed.center(), Pos2::new(100.0, 60.0));
    assert_eq!(scene.timeline().layers().len(), 2);
}

#[test]
fn hits_resolve_to_the_owning_scene_level_span() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 24, 0.0, 0.0, 10.0, 10.0));
    let (commands, symbol) =
        convert_to_symbol(f.session.document(), f.scene, at(&f, 0)).expect("convert");
    f.session.apply(commands).expect("convert");
    let shape_layer = f.layer;
    let commands = place_instance(
        f.session.document(),
        f.scene,
        symbol,
        DVec2::new(100.0, 100.0),
    )
    .expect("place");
    f.session.apply(commands).expect("place");

    let doc = f.session.document();
    let frame = flare_eval::evaluate_frame(doc, f.scene, 3).expect("eval");
    // Paint order: the new layer (last) is drawn first, so the original instance is on top.
    for drawn in &frame.shapes {
        let found = locate_hit(doc, f.scene, 3, drawn).expect("located");
        assert_eq!(found.start, 0);
        assert!(
            doc.scenes()[0]
                .timeline()
                .layers()
                .iter()
                .any(|l| l.id() == found.layer)
        );
    }
    assert!(
        frame
            .shapes
            .iter()
            .any(|d| locate_hit(doc, f.scene, 3, d).is_some_and(|r| r.layer == shape_layer))
    );
}

mod more;
