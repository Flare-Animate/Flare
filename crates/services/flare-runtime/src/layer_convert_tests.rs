#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, Rgba8, ShapeId};
use flare_document::{FrameSpan, Layer, Scene, Shape, Stage, Timeline};

use super::*;

const RED: Rgba8 = Rgba8::opaque(255, 0, 0);

/// A 100 by 60 stage with one layer: a red 20 by 10 rectangle at (10, 20) on frames 1 to 3 and
/// a second keyframe on frame 4 with another at (50, 20).
fn document(layer: Layer) -> (Document, SceneId, LayerId) {
    let id = layer.id();
    let scene = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(100, 60, Rgba8::WHITE).expect("stage"));
    (document, scene, id)
}

fn rect_at(start: u32, duration: u32, x: f64) -> FrameSpan {
    let shape = Shape::rect(ShapeId::new(), x, 20.0, 20.0, 10.0, RED).expect("rect");
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Shape(shape),
    )
}

fn two_rects() -> Layer {
    Layer::new(
        LayerId::new(),
        "Ink",
        vec![rect_at(0, 3, 10.0), rect_at(3, 2, 50.0)],
    )
    .expect("layer")
}

fn applied(document: &Document, commands: Vec<Command>) -> Document {
    apply_all(document, commands)
        .expect("valid")
        .expect("changed")
}

#[test]
fn a_vector_layer_becomes_paintings_and_traces_back_to_shapes_looking_the_same() {
    let (doc, scene, layer) = document(two_rects());
    let commands = convert_layer(&doc, scene, layer, true, "Painting").expect("to raster");
    let raster = applied(&doc, commands);
    let converted = &raster.scenes()[0].timeline().layers()[0];
    assert!(converted.is_raster());
    assert_eq!(converted.spans().len(), 2, "every keyframe kept");
    // The two rectangles are alike, so their paintings are the same pixels, kept once: converting
    // back must still take it out of the library only after the second keyframe.
    assert_eq!(raster.bitmaps().len(), 1, "alike paintings are stored once");
    for span in converted.spans() {
        assert!(matches!(span.content(), Content::Bitmap(_)));
    }
    // Drawn as before: the stage picture at frame 0 is red inside the rectangle, white outside.
    let draw = |d: &Document| {
        let frame = flare_eval::evaluate_frame(d, scene, 0).expect("eval");
        flare_render::render(&frame, 100, 60, Rgba8::WHITE).expect("render")
    };
    let picture = draw(&raster);
    assert_eq!(picture.pixel(20, 25), Some(RED));
    assert_eq!(picture.pixel(5, 25), Some(Rgba8::WHITE));

    let commands = convert_layer(&raster, scene, layer, false, "Painting").expect("to vector");
    let vector = applied(&raster, commands);
    let back = &vector.scenes()[0].timeline().layers()[0];
    assert!(!back.is_raster());
    assert!(
        vector.bitmaps().is_empty(),
        "the paintings left the library"
    );
    assert!(
        back.spans()
            .iter()
            .all(|s| s.elements().all(|c| matches!(c, Content::Shape(_))))
    );
    let picture = draw(&vector);
    assert_eq!(picture.pixel(20, 25), Some(RED));
    assert_eq!(picture.pixel(5, 25), Some(Rgba8::WHITE));
}

#[test]
fn the_same_kind_is_nothing_to_do_and_an_empty_layer_only_switches() {
    let (doc, scene, layer) = document(two_rects());
    assert!(
        convert_layer(&doc, scene, layer, false, "P")
            .expect("already vector")
            .is_empty()
    );
    let empty = Layer::new(LayerId::new(), "Empty", vec![]).expect("layer");
    let (doc, scene, layer) = document(empty);
    let commands = convert_layer(&doc, scene, layer, true, "P").expect("to raster");
    assert_eq!(commands.len(), 1, "only the kind");
    assert!(applied(&doc, commands).scenes()[0].timeline().layers()[0].is_raster());
}

#[test]
fn a_locked_layer_a_folder_and_a_tween_are_refused() {
    let (doc, scene, layer) = document(two_rects().with_locked(true));
    assert_eq!(
        convert_layer(&doc, scene, layer, true, "P"),
        Err(LayerConvertError::Locked)
    );
    let folder = Layer::new(LayerId::new(), "F", vec![])
        .expect("layer")
        .with_folder(true);
    let (doc, scene, layer) = document(folder);
    assert!(matches!(
        convert_layer(&doc, scene, layer, true, "P"),
        Err(LayerConvertError::Refused(_))
    ));
    let tweened = Layer::new(
        LayerId::new(),
        "T",
        vec![
            rect_at(0, 3, 10.0).with_tween(Tween::Shape),
            rect_at(3, 2, 50.0),
        ],
    )
    .expect("layer");
    let (doc, scene, layer) = document(tweened);
    assert!(matches!(
        convert_layer(&doc, scene, layer, true, "P"),
        Err(LayerConvertError::ToRaster { frame: 1, .. })
    ));
}
