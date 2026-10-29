//! Shared test helpers; the tests themselves are in the modules below.
#![allow(clippy::expect_used)]

use super::*;
use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Anchor, Attachment, Content, FrameSpan, Gradient, Instance, Layer, PathSegment, Placement,
    Scene, Shape, Stage, Stroke, Symbol, Timeline, Tween,
};
use glamx::DAffine2;
use std::num::NonZeroU32;

mod failures;
mod roundtrip;

fn sample() -> Document {
    let art = SymbolId::new();
    let sound = flare_core::SoundId::new();
    let transform = Transform2D::new(DAffine2::from_cols_array(&[
        1.0, 0.5, -0.25, 0.0, 3.5, -2.0,
    ]))
    .expect("finite");
    let place = |start| {
        let instance = Instance::new(InstanceId::new(), art, transform);
        FrameSpan::new(
            start,
            NonZeroU32::new(4).expect("nonzero"),
            Content::Instance(instance),
        )
    };
    let empty = FrameSpan::new(8, NonZeroU32::new(2).expect("nonzero"), Content::Empty);
    let curve = Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(DocumentPoint::new(0.0, 0.5).expect("finite")),
            PathSegment::CubicTo(
                DocumentPoint::new(1.0, 2.0).expect("finite"),
                DocumentPoint::new(3.0, -4.0).expect("finite"),
                DocumentPoint::new(5.5, 6.25).expect("finite"),
            ),
            PathSegment::Close,
        ],
        Rgba8::new(10, 20, 30, 40),
    )
    .expect("shape")
    .with_stroke(Stroke::new(2.5, Rgba8::new(9, 8, 7, 255)).expect("stroke"))
    .with_gradient(
        Gradient::linear(
            DocumentPoint::new(-1.5, 0.0).expect("finite"),
            DocumentPoint::new(6.0, 2.25).expect("finite"),
            Rgba8::new(255, 0, 0, 200),
            Rgba8::new(0, 0, 255, 100),
        )
        .expect("gradient"),
    );
    let drawn = FrameSpan::new(
        10,
        NonZeroU32::new(1).expect("nonzero"),
        Content::Shape(curve),
    );
    let layer_id = LayerId::new();
    let layer = Layer::new(
        layer_id,
        "Layer 1",
        vec![place(0).with_tween(Tween::Linear), place(4), empty, drawn],
    )
    .expect("layer")
    .with_visible(false)
    .with_locked(true)
    .with_mask(true)
    .with_cues(vec![flare_document::SoundCue { start: 3, sound }])
    .expect("cues");
    let (scene_one, scene_two) = (SceneId::new(), SceneId::new());
    let scenes = vec![
        Scene::new(scene_one, "One", Timeline::new(vec![layer])),
        Scene::new(scene_two, "Two", Timeline::default()),
    ];
    let symbol = Symbol::new(art, "Art", Timeline::default());
    let notes = vec![
        Attachment::new(
            "xfl",
            Anchor::Document,
            Placement::Child,
            "<media><x a=\"1 &amp; 2\">text</x></media>",
        )
        .expect("doc"),
        Attachment::new(
            "xfl",
            Anchor::Scene(scene_one),
            Placement::LayerAt { index: 3 },
            "<DOMLayer layerType=\"mask\"/>",
        )
        .expect("scene"),
        Attachment::new(
            "xfl",
            Anchor::Layer(layer_id),
            Placement::FrameElement { frame: 4 },
            "<DOMStaticText>Hello \u{1F600}</DOMStaticText>",
        )
        .expect("frame"),
        Attachment::new(
            "xfl",
            Anchor::Layer(layer_id),
            Placement::FrameExtra { frame: 0 },
            "<Actionscript><script><![CDATA[stop();]]></script></Actionscript>",
        )
        .expect("extra"),
        Attachment::new("xfl", Anchor::Symbol(art), Placement::Child, "<scaleGrid/>")
            .expect("symbol"),
    ];
    let _ = scene_two;
    Document::new(
        DocumentId::new(),
        FrameRate::new(30_000, 1_001).expect("rate"),
        scenes,
        vec![symbol],
    )
    .expect("document")
    .with_attachments(notes)
    .expect("attachments")
    .with_sounds(vec![
        flare_document::Sound::new(sound, "Clap", vec![0, 1, 2, 254, 255], 44_100, 2, 3)
            .expect("sound"),
    ])
    .expect("sounds")
    .with_stage(Stage::new(640, 360, Rgba8::new(1, 2, 3, 4)).expect("stage"))
}

/// Deterministic pseudo-random numbers, so a failure is reproducible.
fn lcg(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state >> 33
}

fn attachment_rows(path: &std::path::Path) -> Vec<(String, String)> {
    let conn = Connection::open(path).expect("open");
    let mut statement = conn
        .prepare("SELECT id, payload_json FROM entity WHERE kind = 'attachment' ORDER BY ordinal")
        .expect("prepare");
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows")
}

fn sound_row(path: &std::path::Path) -> (String, String) {
    let conn = Connection::open(path).expect("open");
    conn.query_row(
        "SELECT id, payload_json FROM entity WHERE kind = 'sound'",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .expect("a sound row")
}
