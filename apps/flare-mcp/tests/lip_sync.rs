//! Lip sync through the MCP server: a mouth keyed to a voice, with the built-in analysis and with
//! a stand-in for Rhubarb Lip Sync, and what it refuses.
#![allow(clippy::expect_used)]

mod common;

use std::num::NonZeroU32;

use common::{Fixture, serve_with};
use flare_core::{
    DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SoundId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PlayMode, Scene, Shape, Sound, SoundCue, Symbol,
    SymbolKind, Timeline,
};
use serde_json::json;

/// At 10 fps: a ten-drawing mouth Graphic keyed at frame 0 on the first layer, and a voice cued
/// at 0 that is silent for half a second and then hums low for half a second.
fn mouth_document() -> Document {
    let rate = 8000;
    let samples: Vec<i16> = (0..rate)
        .map(|i| match i < rate / 2 {
            true => 0,
            false => {
                ((std::f32::consts::TAU * 220.0 * i as f32 / rate as f32).sin() * 20_000.0) as i16
            }
        })
        .collect();
    let wav = flare_audio::wav_pcm16(&samples, rate, 1).expect("wav");
    let (symbol, sound) = (SymbolId::new(), SoundId::new());
    let drawings = (0..10)
        .map(|i| {
            let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 9.0, 9.0, Rgba8::opaque(0, 0, 0))
                .expect("rect");
            FrameSpan::new(i, NonZeroU32::MIN, Content::Shape(shape))
        })
        .collect();
    let mouths = Symbol::new(
        symbol,
        "Mouth",
        Timeline::new(vec![
            Layer::new(LayerId::new(), "M", drawings).expect("layer"),
        ]),
    )
    .with_kind(SymbolKind::Graphic);
    let key = FrameSpan::new(
        0,
        NonZeroU32::MIN,
        Content::Instance(Instance::new(
            InstanceId::new(),
            symbol,
            Transform2D::IDENTITY,
        )),
    );
    let layers = vec![
        Layer::new(LayerId::new(), "Mouth", vec![key]).expect("layer"),
        Layer::new(LayerId::new(), "Voice", vec![])
            .expect("layer")
            .with_cues(vec![SoundCue { start: 0, sound }])
            .expect("cue"),
    ];
    Document::new(
        DocumentId::new(),
        FrameRate::new(10, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(layers))],
        vec![mouths],
    )
    .expect("document")
    .with_sounds(vec![
        Sound::new(sound, "voice", wav, rate, 1, u64::from(rate)).expect("sound"),
    ])
    .expect("sounds")
}

async fn server(write: bool, rhubarb: Option<std::path::PathBuf>) -> Fixture {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    flare_storage::save(&mouth_document(), &path).expect("save");
    serve_with(dir, path, write, false, |s| s.with_rhubarb(rhubarb)).await
}

/// Saves, and gives the drawing each frame of the mouth layer shows.
async fn drawings(f: &Fixture) -> Vec<u32> {
    f.ok("save", json!({})).await;
    let document = flare_storage::load(&f.path).expect("load");
    document.scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .flat_map(|span| {
            let Content::Instance(i) = span.content() else {
                panic!("instance");
            };
            assert_eq!(i.playback().mode, PlayMode::SingleFrame);
            std::iter::repeat_n(i.playback().first_frame, span.duration().get() as usize)
        })
        .collect()
}

#[tokio::test]
async fn a_mouth_is_keyed_to_the_voice_in_one_undo_step() {
    let f = server(true, None).await;
    let before = f.revision().await;
    let reply = f.ok("lip_sync", json!({ "frame": 0, "map": "o=7" })).await;
    assert_eq!(reply["analysis"], "built-in", "no Rhubarb here");
    assert_eq!(reply["keyframes"], 2);
    assert_eq!(reply["shapes"][9], "o");
    assert!(reply["revision"].as_u64() > Some(before));
    assert_eq!(drawings(&f).await, [0, 0, 0, 0, 0, 7, 7, 7, 7, 7]);
}

#[tokio::test]
async fn lip_sync_says_why_it_cannot_and_needs_write_access() {
    let f = server(true, None).await;
    for (arguments, says) in [
        (json!({ "frame": 3 }), "no keyframe"),
        (
            json!({ "frame": 0, "analysis": "rhubarb" }),
            "not installed",
        ),
        (json!({ "frame": 0, "analysis": "guess" }), "built-in"),
        (json!({ "frame": 0, "sound": "song" }), "song"),
        (json!({ "frame": 0, "map": "mbp" }), "mbp=3"),
    ] {
        let (is_error, message) = f.tool("lip_sync", arguments.clone()).await;
        assert!(is_error, "{arguments}");
        assert!(message.to_string().contains(says), "{arguments}: {message}");
    }
    let read_only = server(false, None).await;
    let (is_error, _) = read_only.tool("lip_sync", json!({ "frame": 0 })).await;
    assert!(is_error);
}

#[cfg(unix)]
#[tokio::test]
async fn rhubarbs_cues_are_used_when_it_is_there() {
    use std::os::unix::fs::PermissionsExt;
    let programs = tempfile::tempdir().expect("dir");
    let program = programs.path().join("rhubarb");
    std::fs::write(
        &program,
        "#!/bin/sh\necho '{\"mouthCues\":[{\"start\":0,\"end\":0.2,\"value\":\"X\"},{\"start\":0.2,\"end\":1,\"value\":\"F\"}]}'\n",
    )
    .expect("write");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let f = server(true, Some(program)).await;
    let reply = f.ok("lip_sync", json!({ "frame": 0 })).await;
    assert_eq!(reply["analysis"], "rhubarb");
    assert_eq!(drawings(&f).await, [0, 0, 5, 5, 5, 5, 5, 5, 5, 5]);
}
