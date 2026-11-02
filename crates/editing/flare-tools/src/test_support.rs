#![allow(clippy::expect_used)]

use crate::common::SpanRef;
use crate::ops::{find_span, span_bounds};
use emath::Rect;
use flare_commands::Command;
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, PathSegment, Scene, Shape, Timeline};
use flare_history::EditorSession;
use std::num::NonZeroU32;

pub(crate) const RED: Rgba8 = Rgba8::opaque(255, 0, 0);

pub(crate) struct Fixture {
    pub(crate) session: EditorSession,
    pub(crate) scene: SceneId,
    pub(crate) layer: LayerId,
}

pub(crate) fn fixture() -> Fixture {
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

pub(crate) fn rect_span(start: u32, duration: u32, x: f64, y: f64, w: f64, h: f64) -> FrameSpan {
    let shape = Shape::rect(ShapeId::new(), x, y, w, h, RED).expect("rect");
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Shape(shape),
    )
}

pub(crate) fn add(f: &mut Fixture, span: FrameSpan) {
    let command = Command::InsertSpan {
        scene: f.scene,
        layer: f.layer,
        span,
    };
    f.session.apply([command]).expect("insert");
}

pub(crate) fn at(f: &Fixture, start: u32) -> SpanRef {
    SpanRef {
        layer: f.layer,
        start,
        element: 0,
    }
}

pub(crate) fn bounds(f: &Fixture, start: u32) -> Rect {
    span_bounds(f.session.document(), f.scene, at(f, start)).expect("bounds")
}

/// A second layer holding a rectangle span, as another selectable span.
pub(crate) fn add_on_new_layer(f: &mut Fixture, span: FrameSpan) -> SpanRef {
    let layer = LayerId::new();
    f.session
        .apply([
            Command::AddLayer {
                scene: f.scene,
                layer: Layer::new(layer, "M", vec![]).expect("layer"),
            },
            Command::InsertSpan {
                scene: f.scene,
                layer,
                span: span.clone(),
            },
        ])
        .expect("insert");
    SpanRef {
        layer,
        start: span.start(),
        element: 0,
    }
}

pub(crate) fn path_of(f: &Fixture, start: u32) -> Vec<PathSegment> {
    let Content::Shape(shape) = find_span(f.session.document(), f.scene, at(f, start))
        .expect("span")
        .content()
    else {
        panic!("shape");
    };
    shape.path().to_vec()
}

pub(crate) fn anchor_count(f: &Fixture) -> usize {
    path_of(f, 0)
        .iter()
        .filter(|s| !matches!(s, PathSegment::Close))
        .count()
}

pub(crate) fn rect_fixture() -> Fixture {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    f
}

pub(crate) fn circle_fixture() -> Fixture {
    let mut f = fixture();
    let circle = Shape::ellipse(ShapeId::new(), 50.0, 50.0, 20.0, 20.0, RED).expect("circle");
    add(
        &mut f,
        FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(circle)),
    );
    f
}

pub(crate) fn run(f: &mut Fixture, commands: Option<Vec<Command>>) {
    f.session.apply(commands.expect("an edit")).expect("apply");
}

/// Three rectangles in one keyframe of the fixture's layer, 10 wide each, at x = 0, 40 and 80.
pub(crate) fn add_three(f: &mut Fixture) {
    let first = rect_span(0, 6, 0.0, 0.0, 10.0, 10.0);
    let extras = [40.0, 80.0]
        .into_iter()
        .map(|x| rect_span(0, 6, x, 0.0, 10.0, 10.0).content().clone())
        .collect();
    add(f, first.with_extras(extras).expect("extras"));
}

/// The element `index` of the fixture's keyframe at `start`.
pub(crate) fn element(f: &Fixture, start: u32, index: usize) -> SpanRef {
    at(f, start).with_element(index)
}
