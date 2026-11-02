//! A shape tween is baked into one drawing per step, in both outputs.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline, Tween};
use flare_publish::{Html5Options, PublishOptions, publish_html5, publish_swf};

fn tweened(duration: u32) -> Document {
    let shape = |x: f64| {
        Content::Shape(Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, Rgba8::WHITE).expect("rect"))
    };
    let span = |start: u32, length: u32, content| {
        FrameSpan::new(start, NonZeroU32::new(length).expect("nonzero"), content)
    };
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![
            span(0, duration, shape(0.0)).with_tween(Tween::Shape),
            span(duration, 1, shape(100.0)),
        ],
    )
    .expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document")
}

#[test]
fn the_page_has_a_different_drawing_for_each_step() {
    let published = publish_html5(&tweened(5), &Html5Options::default()).expect("publish");
    let open = "<script id=\"flare-data\" type=\"application/json\">";
    let start = published.html.find(open).expect("data") + open.len();
    let end = start + published.html[start..].find("</script>").expect("end");
    let data: serde_json::Value = serde_json::from_str(&published.html[start..end]).expect("json");
    // Five steps and the last keyframe: six pictures, six shapes (one per step).
    assert_eq!(data["frames"].as_array().expect("frames").len(), 6);
    assert_eq!(data["shapes"].as_array().expect("shapes").len(), 6);
    let paths: std::collections::HashSet<&str> = data["shapes"]
        .as_array()
        .expect("shapes")
        .iter()
        .map(|s| s["d"].as_str().expect("path"))
        .collect();
    assert_eq!(paths.len(), 6, "no step reuses another's outline");
}

#[test]
fn the_movie_defines_a_drawing_for_each_step_instead_of_repeating_the_first() {
    let published = publish_swf(&tweened(5), &PublishOptions { compress: false }).expect("publish");
    let movie = swf::decompress_swf(&published.bytes[..]).expect("decompress");
    let parsed = swf::parse_swf(&movie).expect("parse");
    let defined = parsed
        .tags
        .iter()
        .filter(|tag| matches!(tag, swf::Tag::DefineShape(_)))
        .count();
    assert_eq!(defined, 6);
}
