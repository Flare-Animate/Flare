//! What a document may say about its camera.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
};
use flare_document::{
    Camera, Content, Diagnostic, Document, FrameSpan, Layer, Scene, Shape, Symbol, Timeline,
};

fn key() -> FrameSpan {
    let camera =
        Camera::new(DocumentPoint::new(5.0, 5.0).expect("finite"), 2.0, 0.0).expect("camera");
    FrameSpan::new(0, NonZeroU32::MIN, Content::Camera(camera))
}

fn shape_span() -> FrameSpan {
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 4.0, 4.0, Rgba8::WHITE).expect("rect");
    FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape))
}

fn camera_layer(spans: Vec<FrameSpan>) -> Layer {
    Layer::new(LayerId::new(), "Camera", spans)
        .expect("layer")
        .with_camera(true)
}

fn document(
    scene_layers: Vec<Layer>,
    symbol_layers: Vec<Layer>,
) -> Result<Document, Vec<Diagnostic>> {
    let symbols = if symbol_layers.is_empty() {
        vec![]
    } else {
        vec![Symbol::new(
            SymbolId::new(),
            "S",
            Timeline::new(symbol_layers),
        )]
    };
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(scene_layers))],
        symbols,
    )
}

#[test]
fn a_scene_may_have_one_camera_layer_of_camera_keyframes() {
    assert!(document(vec![camera_layer(vec![key()])], vec![]).is_ok());
    assert!(
        document(vec![camera_layer(vec![])], vec![]).is_ok(),
        "a camera with no keyframes yet"
    );
}

#[test]
fn a_second_camera_is_refused() {
    let both = document(
        vec![camera_layer(vec![key()]), camera_layer(vec![key()])],
        vec![],
    );
    assert!(
        both.expect_err("two cameras")
            .iter()
            .any(|d| matches!(d, Diagnostic::TooManyCameras(_)))
    );
}

#[test]
fn a_camera_in_a_symbol_is_refused() {
    let found = document(vec![], vec![camera_layer(vec![key()])]);
    assert!(
        found
            .expect_err("camera in a symbol")
            .iter()
            .any(|d| matches!(d, Diagnostic::CameraInSymbol(_)))
    );
}

#[test]
fn a_camera_layer_holds_only_camera_keyframes_and_they_stay_on_a_camera_layer() {
    let drawn = document(vec![camera_layer(vec![shape_span()])], vec![]);
    assert!(
        drawn
            .expect_err("art on the camera")
            .iter()
            .any(|d| matches!(d, Diagnostic::CameraContent(_)))
    );
    let plain = Layer::new(LayerId::new(), "L", vec![key()]).expect("layer");
    assert!(
        document(vec![plain], vec![])
            .expect_err("keyframes on a plain layer")
            .iter()
            .any(|d| matches!(d, Diagnostic::CameraKeyframesElsewhere(_)))
    );
}

#[test]
fn a_camera_cannot_also_be_a_mask_a_guide_or_a_folder() {
    for layer in [
        camera_layer(vec![]).with_mask(true),
        camera_layer(vec![]).with_guide(true),
    ] {
        assert!(
            document(vec![layer], vec![])
                .expect_err("conflict")
                .iter()
                .any(|d| matches!(d, Diagnostic::CameraConflict(_)))
        );
    }
}
