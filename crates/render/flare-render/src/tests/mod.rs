//! Shared test helpers; the tests themselves are in the modules below.
#![allow(clippy::expect_used)]

use super::*;
use flare_core::InstanceId;
use flare_core::{DocumentId, FrameRate, LayerId, SceneId, ShapeId, SymbolId, Transform2D};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, Scene, Shape, Symbol, Timeline,
};
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;

mod masks;
mod shapes;
mod viewport;

const RED: Rgba8 = Rgba8::opaque(255, 0, 0);

fn span(content: Content) -> FrameSpan {
    FrameSpan::new(0, NonZeroU32::new(1).expect("nonzero"), content)
}

fn scene_with(spans: Vec<FrameSpan>, symbols: Vec<Symbol>) -> (Document, SceneId) {
    let layers = spans
        .into_iter()
        .map(|s| Layer::new(LayerId::new(), "L", vec![s]).expect("layer"))
        .collect();
    let id = SceneId::new();
    let scene = Scene::new(id, "S", Timeline::new(layers));
    let rate = FrameRate::new(24, 1).expect("rate");
    (
        Document::new(DocumentId::new(), rate, vec![scene], symbols).expect("doc"),
        id,
    )
}

fn scene_with_layers(layers: Vec<Layer>) -> (Document, SceneId) {
    let id = SceneId::new();
    let scene = Scene::new(id, "S", Timeline::new(layers));
    let rate = FrameRate::new(24, 1).expect("rate");
    (
        Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc"),
        id,
    )
}

fn rect(x: f64, y: f64, w: f64, h: f64, fill: Rgba8) -> Content {
    Content::Shape(Shape::rect(ShapeId::new(), x, y, w, h, fill).expect("rect"))
}

fn draw(doc: &Document, scene: SceneId, bg: Rgba8) -> Image {
    draw_sized(doc, scene, bg, 20, 20)
}

fn draw_sized(doc: &Document, scene: SceneId, bg: Rgba8, w: u32, h: u32) -> Image {
    let frame = flare_eval::evaluate_frame(doc, scene, 0).expect("eval");
    render(&frame, w, h, bg).expect("render")
}

fn gradient_rect(w: f64, h: f64) -> Shape {
    use flare_core::DocumentPoint;
    let end = DocumentPoint::new(w, 0.0).expect("finite");
    let start = DocumentPoint::new(0.0, 0.0).expect("finite");
    let gradient = flare_document::Gradient::linear(
        start,
        end,
        Rgba8::opaque(255, 0, 0),
        Rgba8::opaque(0, 0, 255),
    )
    .expect("gradient");
    Shape::rect(ShapeId::new(), 0.0, 0.0, w, h, Rgba8::WHITE)
        .expect("rect")
        .with_gradient(gradient)
}

/// A mask layer over a layer holding one rectangle.
fn masked_doc(
    mask_rect: (f64, f64, f64, f64),
    art_rect: (f64, f64, f64, f64),
) -> (Document, SceneId) {
    let layer = |name: &str, r: (f64, f64, f64, f64), mask: bool| {
        Layer::new(
            LayerId::new(),
            name,
            vec![span(rect(r.0, r.1, r.2, r.3, RED))],
        )
        .expect("layer")
        .with_mask(mask)
    };
    scene_with_layers(vec![
        layer("mask", mask_rect, true),
        layer("art", art_rect, false),
    ])
}
