//! A saved project comes back identical, whatever it held. Generated documents with strokes of
//! every style, gradients, flags and spans go through a real file.
#![cfg(not(target_arch = "wasm32"))]
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, DocumentPoint, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{
    Cap, Content, Document, FrameSpan, Gradient, Join, Layer, Scene, Shape, Stage, Stroke, Timeline,
};
use proptest::prelude::*;

fn color() -> impl Strategy<Value = Rgba8> {
    any::<[u8; 4]>().prop_map(|[r, g, b, a]| Rgba8::new(r, g, b, a))
}

fn cap() -> impl Strategy<Value = Cap> {
    prop_oneof![Just(Cap::Butt), Just(Cap::Round), Just(Cap::Square)]
}

fn join() -> impl Strategy<Value = Join> {
    prop_oneof![Just(Join::Round), Just(Join::Bevel), Just(Join::Miter)]
}

#[derive(Debug, Clone)]
struct ShapeSpec {
    rect: (f64, f64, f64, f64),
    fill: Rgba8,
    stroke: Option<(f64, Rgba8, Cap, Join)>,
    gradient: Option<Rgba8>,
}

fn shape_spec() -> impl Strategy<Value = ShapeSpec> {
    (
        (
            -500.0_f64..500.0,
            -500.0_f64..500.0,
            1.0_f64..300.0,
            1.0_f64..300.0,
        ),
        color(),
        prop::option::of((0.1_f64..40.0, color(), cap(), join())),
        prop::option::of(color()),
    )
        .prop_map(|(rect, fill, stroke, gradient)| ShapeSpec {
            rect,
            fill,
            stroke,
            gradient,
        })
}

fn build(spec: &ShapeSpec) -> Shape {
    let (x, y, w, h) = spec.rect;
    let mut shape = Shape::rect(ShapeId::new(), x, y, w, h, spec.fill).expect("rect");
    if let Some((width, color, cap, join)) = spec.stroke {
        shape = shape.with_stroke(
            Stroke::new(width, color)
                .expect("stroke")
                .with_cap(cap)
                .with_join(join),
        );
    }
    if let Some(end) = spec.gradient {
        let gradient = Gradient::linear(
            DocumentPoint::new(x, y).expect("finite"),
            DocumentPoint::new(x + w, y).expect("finite"),
            spec.fill,
            end,
        )
        .expect("gradient");
        shape = shape.with_gradient(gradient);
    }
    shape
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(48))]

    #[test]
    fn whatever_was_saved_is_what_comes_back(
        layers in prop::collection::vec(
            (any::<bool>(), any::<bool>(), 0_u32..30, 1_u32..30, shape_spec()),
            1..8,
        ),
        stage in (1_u32..2000, 1_u32..2000, color()),
        rate in (1_u32..120, 1_u32..4),
    ) {
        let built: Vec<Layer> = layers
            .iter()
            .enumerate()
            .map(|(i, (visible, locked, start, length, spec))| {
                let span = FrameSpan::new(
                    *start,
                    NonZeroU32::new(*length).expect("nonzero"),
                    Content::Shape(build(spec)),
                );
                Layer::new(LayerId::new(), format!("Layer {i}"), vec![span])
                    .expect("layer")
                    .with_visible(*visible)
                    .with_locked(*locked)
            })
            .collect();
        let document = Document::new(
            DocumentId::new(),
            FrameRate::new(rate.0, rate.1).expect("rate"),
            vec![Scene::new(SceneId::new(), "S", Timeline::new(built))],
            vec![],
        )
        .expect("document")
        .with_stage(Stage::new(stage.0, stage.1, stage.2).expect("stage"));
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("p.flare");
        flare_storage::save(&document, &path).expect("save");
        prop_assert_eq!(flare_storage::load(&path).expect("load"), document);
    }
}
