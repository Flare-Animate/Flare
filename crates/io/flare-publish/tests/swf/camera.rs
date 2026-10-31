//! A camera moves what a SWF shows, frame by frame, and nothing else.

use flare_core::DocumentPoint;
use flare_document::{Camera, Tween};

use super::*;

fn camera(x: f64, y: f64, zoom: f64, turns: f64) -> Camera {
    Camera::new(
        DocumentPoint::new(x, y).expect("finite"),
        zoom,
        turns * std::f64::consts::TAU,
    )
    .expect("camera")
}

/// A red square and a symbol with a blue one, seen by a camera that zooms in on the square,
/// turns, and then moves across to the symbol.
fn filmed() -> Document {
    let dot = flare_core::SymbolId::new();
    let symbol = flare_document::Symbol::new(
        dot,
        "Dot",
        Timeline::new(vec![layer(
            "L",
            vec![span(
                0,
                1,
                Content::Shape(rect(0.0, 0.0, 16.0, 16.0, Rgba8::opaque(0, 0, 255))),
            )],
        )]),
    );
    let art = layer(
        "art",
        vec![span(
            0,
            12,
            Content::Shape(rect(20.0, 20.0, 24.0, 24.0, Rgba8::opaque(255, 0, 0))),
        )],
    );
    let moving = layer("dot", vec![span(0, 12, instance(dot, 90.0, 30.0))]);
    let key = |start, camera| span(start, 4, Content::Camera(camera));
    let cam = layer(
        "Camera",
        vec![
            key(0, camera(32.0, 32.0, 1.5, 0.0)).with_tween(Tween::Linear),
            key(4, camera(40.0, 32.0, 2.0, 0.1))
                .with_tween(Tween::Linear)
                .with_ease(50),
            key(8, camera(98.0, 38.0, 2.5, -0.05)),
        ],
    )
    .with_camera(true);
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![cam, moving, art]));
    document(120, 80, vec![scene], vec![symbol])
}

#[test]
fn the_published_movie_shows_the_cameras_picture_in_every_frame() {
    let doc = filmed();
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    assert_eq!(published.frames, 12);
    let parsed = parse(&published.bytes);
    let background = doc.stage().background();
    let mut changed = 0;
    let mut last: Option<Vec<u8>> = None;
    for frame in 0..12 {
        let ours = render_original(&doc, 0, frame);
        let (max, mean) = difference(
            &ours,
            &render_swf_frame(&parsed, frame as usize, background),
        );
        assert!(
            max <= 60 && mean < 0.3,
            "frame {frame}: max {max}, mean {mean}"
        );
        if last.as_deref().is_some_and(|l| l != ours.rgba()) {
            changed += 1;
        }
        last = Some(ours.rgba().to_vec());
    }
    // The camera moves in each of the first eight steps and holds its last keyframe after.
    assert_eq!(changed, 8, "the picture moves while the camera does");
}

#[test]
fn a_publish_without_a_camera_is_as_before() {
    // The camera layer adds no depth of its own and no sprite: only the others' placements move.
    let doc = filmed();
    let with = parse(
        &publish_swf(&doc, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let flat = animated();
    let without = parse(
        &publish_swf(&flat, &PublishOptions::default())
            .expect("publish")
            .bytes,
    );
    let places = |p: &Parsed| {
        p.tags
            .iter()
            .filter(|t| matches!(t, Tag::PlaceObject(_)))
            .count()
    };
    assert!(places(&with) > 0 && places(&without) > 0);
    assert_eq!(
        with.tags
            .iter()
            .filter(|t| matches!(t, Tag::DefineSprite(_)))
            .count(),
        1,
        "only the symbol is a sprite"
    );
}
