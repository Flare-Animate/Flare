use flare_core::DocumentPoint;
use flare_document::Camera;

use super::*;

/// A red square at (140, 40) to (160, 60) on a 200 by 100 stage, with a camera layer on top
/// holding `camera` for the whole movie.
fn looked_at(camera: Camera) -> (Document, SceneId) {
    let art = Layer::new(
        LayerId::new(),
        "art",
        vec![span(rect(140.0, 40.0, 20.0, 20.0, RED))],
    )
    .expect("layer");
    let cam = Layer::new(
        LayerId::new(),
        "Camera",
        vec![span(Content::Camera(camera))],
    )
    .expect("layer")
    .with_camera(true);
    let (doc, id) = scene_with_layers(vec![cam, art]);
    (
        doc.with_stage(flare_document::Stage::new(200, 100, Rgba8::WHITE).expect("stage")),
        id,
    )
}

fn camera(x: f64, y: f64, zoom: f64, rotation: f64) -> Camera {
    Camera::new(DocumentPoint::new(x, y).expect("finite"), zoom, rotation).expect("camera")
}

#[test]
fn the_render_shows_what_the_camera_sees() {
    // Looking at the square from twice as close: it fills the middle, 40 px across.
    let (doc, id) = looked_at(camera(150.0, 50.0, 2.0, 0.0));
    let image = draw_sized(&doc, id, Rgba8::WHITE, 200, 100);
    assert_eq!(image.pixel(100, 50), Some(RED));
    assert_eq!(image.pixel(81, 31), Some(RED));
    assert_eq!(image.pixel(118, 68), Some(RED));
    assert_eq!(image.pixel(78, 50), Some(Rgba8::WHITE));
    // The square's old place is empty now.
    assert_eq!(image.pixel(150, 50), Some(Rgba8::WHITE));
}

#[test]
fn a_camera_turned_a_quarter_clockwise_turns_the_picture_a_quarter_counterclockwise() {
    // Zoom 1 about the stage's middle. The square's middle is 50 right of the middle, so it
    // ends up 50 above it, at the top edge: a square 20 across, half of it on the stage.
    let (doc, id) = looked_at(camera(100.0, 50.0, 1.0, std::f64::consts::FRAC_PI_2));
    let image = draw_sized(&doc, id, Rgba8::WHITE, 200, 100);
    assert_eq!(image.pixel(100, 0), Some(RED));
    assert_eq!(image.pixel(100, 8), Some(RED));
    assert_eq!(image.pixel(100, 13), Some(Rgba8::WHITE));
    assert_eq!(
        image.pixel(100, 50),
        Some(Rgba8::WHITE),
        "the middle is empty"
    );
    assert_eq!(
        image.pixel(150, 50),
        Some(Rgba8::WHITE),
        "and so is the square's old place"
    );
}

#[test]
fn the_stage_without_a_camera_is_unchanged() {
    let (with, id) = looked_at(camera(100.0, 50.0, 1.0, 0.0));
    let without = {
        let art = Layer::new(
            LayerId::new(),
            "art",
            vec![span(rect(140.0, 40.0, 20.0, 20.0, RED))],
        )
        .expect("layer");
        let (doc, id) = scene_with_layers(vec![art]);
        draw_sized(
            &doc.with_stage(flare_document::Stage::new(200, 100, Rgba8::WHITE).expect("stage")),
            id,
            Rgba8::WHITE,
            200,
            100,
        )
    };
    assert_eq!(
        draw_sized(&with, id, Rgba8::WHITE, 200, 100),
        without,
        "the centered camera changes nothing"
    );
}
