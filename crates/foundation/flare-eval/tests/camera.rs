//! The camera moves what every export and the preview show, and only that.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, DocumentPoint, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{
    Camera, Content, Document, FrameSpan, Layer, Scene, Shape, Stage, Timeline, Tween,
};
use flare_eval::{
    LevelContent, camera_at, camera_view, evaluate_frame, evaluate_frame_without_camera,
    evaluate_level,
};
use glamx::DVec2;

fn camera(x: f64, y: f64, zoom: f64, turns: f64) -> Camera {
    Camera::new(
        DocumentPoint::new(x, y).expect("finite"),
        zoom,
        turns * std::f64::consts::TAU,
    )
    .expect("camera")
}

fn key(start: u32, duration: u32, camera: Camera) -> FrameSpan {
    FrameSpan::new(
        start,
        NonZeroU32::new(duration).expect("nonzero"),
        Content::Camera(camera),
    )
}

fn scene_with(spans: Vec<FrameSpan>, hidden: bool) -> (Document, SceneId) {
    let rect = Shape::rect(
        ShapeId::new(),
        140.0,
        40.0,
        20.0,
        20.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let art = Layer::new(
        LayerId::new(),
        "art",
        vec![FrameSpan::new(
            0,
            NonZeroU32::new(20).expect("nonzero"),
            Content::Shape(rect),
        )],
    )
    .expect("layer");
    let cam = Layer::new(LayerId::new(), "Camera", spans)
        .expect("layer")
        .with_camera(true)
        .with_visible(!hidden);
    let id = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(id, "S", Timeline::new(vec![cam, art]))],
        vec![],
    )
    .expect("document")
    .with_stage(Stage::new(200, 100, Rgba8::WHITE).expect("stage"));
    (document, id)
}

fn corner(document: &Document, id: SceneId, frame: u32, with_camera: bool) -> DVec2 {
    let frame = if with_camera {
        evaluate_frame(document, id, frame)
    } else {
        evaluate_frame_without_camera(document, id, frame)
    }
    .expect("frame");
    assert_eq!(
        frame.shapes.len(),
        1,
        "the camera layer draws nothing itself"
    );
    frame.shapes[0]
        .transform
        .transform_point2(DVec2::new(150.0, 50.0))
}

#[test]
fn a_camera_moves_the_scene_in_every_frame_and_leaves_the_editing_view_alone() {
    // Looking at the red square's middle from twice as close.
    let (document, id) = scene_with(vec![key(0, 20, camera(150.0, 50.0, 2.0, 0.0))], false);
    let seen = corner(&document, id, 3, true);
    assert!((seen - DVec2::new(100.0, 50.0)).length() < 1e-9, "{seen:?}");
    let drawn = corner(&document, id, 3, false);
    assert!(
        (drawn - DVec2::new(150.0, 50.0)).length() < 1e-9,
        "{drawn:?}"
    );
}

#[test]
fn a_motion_tween_moves_the_camera_between_keyframes_with_the_ease() {
    let mut first = key(0, 10, camera(100.0, 50.0, 1.0, 0.0)).with_tween(Tween::Linear);
    let second = key(10, 10, camera(150.0, 50.0, 3.0, 0.25));
    let (document, _) = scene_with(vec![first.clone(), second.clone()], false);
    let layer = &document.scenes()[0].timeline().layers()[0];
    let at = |frame| camera_at(layer, frame).expect("camera");
    assert_eq!(at(0), camera(100.0, 50.0, 1.0, 0.0));
    let halfway = at(5);
    assert!((halfway.center().as_vec() - DVec2::new(125.0, 50.0)).length() < 1e-9);
    assert!((halfway.zoom() - 2.0).abs() < 1e-9);
    assert!((halfway.rotation() - 0.125 * std::f64::consts::TAU).abs() < 1e-9);
    // Held after the second keyframe; no tween on it.
    assert_eq!(at(15), camera(150.0, 50.0, 3.0, 0.25));
    // An ease speeds the start: a quarter of the way in time is less than a quarter of the way.
    first = first.with_ease(-100);
    let (eased, _) = scene_with(vec![first, second], false);
    let layer = &eased.scenes()[0].timeline().layers()[0];
    assert!(camera_at(layer, 2).expect("camera").center().as_vec().x < 110.0);
}

#[test]
fn no_camera_a_hidden_one_or_a_frame_before_its_first_keyframe_is_the_scene_as_drawn() {
    let key_at_5 = vec![key(5, 5, camera(150.0, 50.0, 2.0, 0.0))];
    let (document, id) = scene_with(key_at_5.clone(), false);
    let timeline = document.scenes()[0].timeline();
    assert_eq!(
        camera_view(&document, timeline, 2),
        glamx::DAffine2::IDENTITY
    );
    assert_ne!(
        camera_view(&document, timeline, 6),
        glamx::DAffine2::IDENTITY
    );
    assert_eq!(
        corner(&document, id, 2, true),
        corner(&document, id, 2, false)
    );
    let (hidden, hidden_id) = scene_with(key_at_5, true);
    assert_eq!(corner(&hidden, hidden_id, 6, true), DVec2::new(150.0, 50.0));
}

#[test]
fn a_level_carries_the_view_and_has_no_item_for_the_camera_layer() {
    let (document, _) = scene_with(vec![key(0, 20, camera(150.0, 50.0, 2.0, 0.0))], false);
    let timeline = document.scenes()[0].timeline();
    let level = evaluate_level(&document, timeline, 4).expect("level");
    assert_eq!(level.view, camera_view(&document, timeline, 4));
    assert_eq!(level.items.len(), 1);
    assert!(matches!(level.items[0].content, LevelContent::Shape { .. }));
}
