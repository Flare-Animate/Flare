use super::*;

use std::num::NonZeroU32;

use flare_core::{
    DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SoundId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PlayMode, Scene, Shape, Sound, SoundCue, Symbol,
    SymbolKind, Timeline,
};

/// A project at 10 fps: a ten-drawing mouth Graphic keyed at frame 0 on the first layer, and a
/// voice cued at 0 (half a second of silence, then half a second of a low hum).
fn mouth_project(file: &str) -> LayerId {
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
    let (mouth_layer, symbol, sound) = (LayerId::new(), SymbolId::new(), SoundId::new());
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
        Layer::new(mouth_layer, "Mouth", vec![key]).expect("layer"),
        Layer::new(LayerId::new(), "Voice", vec![])
            .expect("layer")
            .with_cues(vec![SoundCue { start: 0, sound }])
            .expect("cue"),
    ];
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(10, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(layers))],
        vec![mouths],
    )
    .expect("document")
    .with_sounds(vec![
        Sound::new(sound, "voice", wav, rate, 1, u64::from(rate)).expect("sound"),
    ])
    .expect("sounds");
    flare_storage::save(&document, Path::new(file)).expect("save");
    mouth_layer
}

/// The drawing each frame of the mouth layer shows.
fn drawings(file: &str) -> Vec<u32> {
    let document = flare_storage::load(Path::new(file)).expect("load");
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

#[test]
fn lip_sync_keys_the_mouth_to_the_voice_with_the_drawings_mapped() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let layer = mouth_project(&file);
    let reply = json(&[
        "lip-sync",
        &file,
        "--frame",
        "0",
        "--layer",
        &layer.as_uuid().to_string(),
        "--analysis",
        "built-in",
        "--map",
        "neutral=9,o=3",
    ]);
    assert_eq!(reply["analysis"], "built-in");
    assert_eq!(
        (reply["frames"].as_u64(), reply["keyframes"].as_u64()),
        (Some(10), Some(2))
    );
    assert_eq!(reply["shapes"][0], "neutral");
    assert_eq!(reply["shapes"][9], "o");
    assert_eq!(reply["map"]["mbp"], 1);
    assert_eq!(drawings(&file), [9, 9, 9, 9, 9, 3, 3, 3, 3, 3]);
}

#[test]
fn what_cannot_be_lip_synced_is_an_error_saying_why() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    mouth_project(&file);
    let before = std::fs::read(&file).expect("read");
    let fails = |args: &[&str], says: &str| {
        let output = run(args);
        assert!(!output.status.success(), "{args:?}");
        let message = String::from_utf8_lossy(&output.stderr);
        assert!(message.contains(says), "{args:?}: {message}");
    };
    fails(
        &["lip-sync", &file, "--frame", "4", "--analysis", "built-in"],
        "no keyframe",
    );
    fails(
        &["lip-sync", &file, "--frame", "0", "--sound", "song"],
        "song",
    );
    fails(
        &["lip-sync", &file, "--frame", "0", "--map", "ah=2"],
        "not a mouth shape",
    );
    fails(
        &["lip-sync", &file, "--frame", "0", "--analysis", "guess"],
        "built-in",
    );
    assert_eq!(
        std::fs::read(&file).expect("read"),
        before,
        "nothing was written"
    );
}

#[cfg(unix)]
#[test]
fn rhubarb_is_found_through_its_variable_and_its_failure_is_reported() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    mouth_project(&file);
    let program = dir.path().join("rhubarb");
    std::fs::write(&program, "#!/bin/sh\necho 'no voice found' >&2\nexit 1\n").expect("write");
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    let with_rhubarb = |analysis: &str| {
        Command::new(env!("CARGO_BIN_EXE_flare"))
            .args(["lip-sync", &file, "--frame", "0", "--analysis", analysis])
            .env("FLARE_RHUBARB", &program)
            .output()
            .expect("run cli")
    };
    let output = with_rhubarb("rhubarb");
    assert!(!output.status.success());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message.contains("Rhubarb Lip Sync failed: no voice found"),
        "{message}"
    );

    let output = with_rhubarb("auto");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let reply: Value = serde_json::from_slice(&output.stdout).expect("json");
    assert_eq!(reply["analysis"], "built-in");
    assert!(
        reply["fell_back"]
            .as_str()
            .is_some_and(|w| w.contains("no voice found")),
        "{reply}"
    );
}
