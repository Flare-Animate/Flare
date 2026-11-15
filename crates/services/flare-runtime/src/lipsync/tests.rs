#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{
    DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SoundId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PlayMode, Scene, Shape, Sound, SoundCue, Symbol,
    SymbolKind, Timeline,
};
use flare_history::EditorSession;

use super::*;

const RATE: u32 = 8000;

/// Half a second of silence, then half a second of a loud low voice, as a WAV file.
fn voice() -> Vec<u8> {
    let quiet = std::iter::repeat_n(0_i16, RATE as usize / 2);
    let low = (0..RATE / 2).map(|i| {
        ((std::f32::consts::TAU * 220.0 * i as f32 / RATE as f32).sin() * 20_000.0) as i16
    });
    let samples: Vec<i16> = quiet.chain(low).collect();
    flare_audio::wav_pcm16(&samples, RATE, 1).expect("wav")
}

struct Scenario {
    document: Document,
    request: LipSyncRequest,
}

/// A ten-drawing mouth Graphic on a layer from frame 0 for one frame, and the voice cued at 0, at
/// 10 frames a second.
fn scenario(analysis: Analysis) -> Scenario {
    let (scene, mouth_layer, symbol, sound) = (
        SceneId::new(),
        LayerId::new(),
        SymbolId::new(),
        SoundId::new(),
    );
    let drawings = (0..10)
        .map(|i| {
            let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, Rgba8::opaque(0, 0, 0))
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
    let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
    let key = FrameSpan::new(0, NonZeroU32::MIN, Content::Instance(instance));
    let layers = vec![
        Layer::new(mouth_layer, "Mouth", vec![key]).expect("layer"),
        Layer::new(LayerId::new(), "Voice", vec![])
            .expect("layer")
            .with_cues(vec![SoundCue { start: 0, sound }])
            .expect("cue"),
    ];
    let bytes = voice();
    let audio = flare_audio::decode(&bytes).expect("decode");
    let sound_value =
        Sound::new(sound, "Voice", bytes, RATE, 1, audio.frames() as u64).expect("sound");
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(10, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(layers))],
        vec![mouths],
    )
    .expect("document")
    .with_sounds(vec![sound_value])
    .expect("sounds");
    Scenario {
        document,
        request: LipSyncRequest {
            scene,
            at: SpanRef {
                layer: mouth_layer,
                start: 0,
                element: 0,
            },
            sound,
            map: MouthMap::in_order(),
            analysis,
        },
    }
}

/// The first frame each key of the mouth layer shows, frame by frame.
fn drawings(document: &Document) -> Vec<u32> {
    let layer = &document.scenes()[0].timeline().layers()[0];
    layer
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
fn the_built_in_analysis_keys_the_mouth_to_the_voice_as_one_edit() {
    let s = scenario(Analysis::BuiltIn);
    let made = lip_sync(&s.document, &s.request, None).expect("lip sync");
    assert_eq!(made.used, Analysis::BuiltIn);
    assert_eq!(made.shapes.len(), 10);
    assert_eq!((made.start, made.frames), (0, 10));
    let mut session = EditorSession::new(s.document);
    session.apply(made.commands).expect("apply");
    // Silent for five frames (Neutral, frame 0), then the low voice: O, frame 4.
    assert_eq!(drawings(session.document()), [0, 0, 0, 0, 0, 4, 4, 4, 4, 4]);
}

#[test]
fn auto_with_no_rhubarb_uses_the_built_in_analysis_and_rhubarb_alone_says_it_is_missing() {
    let s = scenario(Analysis::Auto);
    let made = lip_sync(&s.document, &s.request, None).expect("lip sync");
    assert_eq!((made.used, made.fell_back), (Analysis::BuiltIn, None));
    let s = scenario(Analysis::Rhubarb);
    let error = lip_sync(&s.document, &s.request, None).expect_err("missing");
    assert_eq!(error, LipSyncFailure::RhubarbMissing);
    assert!(error.to_string().contains(RHUBARB_ENV), "{error}");
}

#[test]
fn a_sound_not_in_the_library_or_not_cued_is_refused() {
    let mut s = scenario(Analysis::BuiltIn);
    s.request.sound = SoundId::new();
    assert!(matches!(
        lip_sync(&s.document, &s.request, None),
        Err(LipSyncFailure::UnknownSound(_))
    ));
    let mut s = scenario(Analysis::BuiltIn);
    s.request.scene = SceneId::new();
    assert_eq!(
        lip_sync(&s.document, &s.request, None),
        Err(LipSyncFailure::Place(LipSyncError::SoundNotCued))
    );
}

#[test]
fn the_sound_is_found_by_name_or_id_or_is_the_one_on_the_timeline() {
    let s = scenario(Analysis::BuiltIn);
    let (document, scene, sound) = (&s.document, s.request.scene, s.request.sound);
    assert_eq!(pick_sound(document, scene, None), Ok(sound));
    assert_eq!(pick_sound(document, scene, Some(" Voice ")), Ok(sound));
    let id = sound.as_uuid().to_string();
    assert_eq!(pick_sound(document, scene, Some(&id)), Ok(sound));
    assert_eq!(
        pick_sound(document, scene, Some("Song")),
        Err(LipSyncFailure::UnknownSound("Song".to_owned()))
    );
    assert_eq!(
        pick_sound(document, SceneId::new(), None),
        Err(LipSyncFailure::NoSound)
    );

    let mut session = EditorSession::new(s.document.clone());
    let other = SoundId::new();
    let voice_layer = s.document.scenes()[0].timeline().layers()[1].id();
    session
        .apply([
            flare_commands::Command::AddSound(
                Sound::new(other, "Other", voice(), RATE, 1, 8000).expect("sound"),
            ),
            flare_commands::Command::SetCues {
                scene,
                layer: voice_layer,
                cues: vec![
                    SoundCue { start: 0, sound },
                    SoundCue {
                        start: 5,
                        sound: other,
                    },
                ],
            },
        ])
        .expect("second sound");
    assert_eq!(
        pick_sound(session.document(), scene, None),
        Err(LipSyncFailure::WhichSound)
    );
}

#[test]
fn an_analysis_is_named_as_the_command_line_and_mcp_server_type_it() {
    for analysis in Analysis::ALL {
        assert_eq!(Analysis::parse(analysis.name()), Some(analysis));
    }
    assert_eq!(Analysis::parse("Built In"), Some(Analysis::BuiltIn));
    assert_eq!(Analysis::parse("builtin"), Some(Analysis::BuiltIn));
    assert_eq!(Analysis::parse("whisper"), None);
}

#[test]
fn a_lip_sync_job_comes_back_with_the_document_it_was_made_for() {
    let s = scenario(Analysis::BuiltIn);
    let document = std::sync::Arc::new(s.document);
    let done = crate::run_audio(crate::AudioJob::LipSync {
        id: 7,
        document: std::sync::Arc::clone(&document),
        request: s.request,
        rhubarb: None,
    });
    let crate::Done::LipSynced {
        id,
        document: made_for,
        result,
    } = done
    else {
        panic!("not a lip sync");
    };
    assert_eq!(id, 7);
    assert!(std::sync::Arc::ptr_eq(&made_for, &document));
    assert_eq!(result.expect("made").keyframes, 2);
}

#[cfg(unix)]
mod with_a_program {
    use super::*;
    use std::path::{Path, PathBuf};

    fn script(dir: &Path, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join("rhubarb");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    #[test]
    fn rhubarbs_cues_are_used_and_it_is_handed_a_wav_after_its_options() {
        let dir = tempfile::tempdir().expect("dir");
        let seen = dir.path().join("seen");
        let program = script(
            dir.path(),
            &format!(
                "echo \"$@\" > '{}'\nhead -c 4 \"$4\" >> '{}'\necho '{{\"mouthCues\":[{{\"start\":0,\"end\":0.3,\"value\":\"D\"}},{{\"start\":0.3,\"end\":1,\"value\":\"A\"}}]}}'",
                seen.display(),
                seen.display()
            ),
        );
        let s = scenario(Analysis::Rhubarb);
        let made = lip_sync(&s.document, &s.request, Some(&program)).expect("lip sync");
        assert_eq!(made.used, Analysis::Rhubarb);
        let mut session = EditorSession::new(s.document);
        session.apply(made.commands).expect("apply");
        assert_eq!(drawings(session.document()), [2, 2, 2, 1, 1, 1, 1, 1, 1, 1]);
        let seen = std::fs::read_to_string(seen).expect("seen");
        assert!(seen.starts_with("--exportFormat json --quiet /"), "{seen}");
        assert!(seen.ends_with("voice.wav\nRIFF"), "{seen}");
    }

    #[test]
    fn a_failing_rhubarb_is_reported_alone_and_auto_falls_back_saying_why() {
        let dir = tempfile::tempdir().expect("dir");
        let program = script(dir.path(), "echo 'cannot read the file' >&2\nexit 3");
        let s = scenario(Analysis::Rhubarb);
        let error = lip_sync(&s.document, &s.request, Some(&program)).expect_err("fails");
        assert_eq!(
            error,
            LipSyncFailure::Rhubarb("cannot read the file".to_owned())
        );
        let s = scenario(Analysis::Auto);
        let made = lip_sync(&s.document, &s.request, Some(&program)).expect("fell back");
        assert_eq!(made.used, Analysis::BuiltIn);
        assert!(
            made.fell_back
                .as_deref()
                .is_some_and(|w| w.contains("cannot read")),
            "{:?}",
            made.fell_back
        );
    }

    #[test]
    fn output_rhubarb_should_not_have_written_is_refused() {
        let dir = tempfile::tempdir().expect("dir");
        let program = script(dir.path(), "echo '{\"mouthCues\":[{\"value\":\"A\"}]}'");
        let s = scenario(Analysis::Rhubarb);
        let error = lip_sync(&s.document, &s.request, Some(&program)).expect_err("bad");
        assert!(matches!(error, LipSyncFailure::Analysis(_)), "{error:?}");
    }

    #[test]
    fn a_rhubarb_that_runs_too_long_is_stopped() {
        let dir = tempfile::tempdir().expect("dir");
        let program = script(dir.path(), "exec sleep 30");
        let audio = flare_audio::decode(&voice()).expect("decode");
        let started = std::time::Instant::now();
        let error =
            rhubarb::run_with_limit(&program, &audio, std::time::Duration::from_millis(200))
                .expect_err("stopped");
        assert!(error.contains("longer than"), "{error}");
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }

    #[test]
    fn a_program_that_is_not_there_is_named_in_the_error() {
        let audio = flare_audio::decode(&voice()).expect("decode");
        let error = rhubarb::run(Path::new("/no/such/rhubarb"), &audio).expect_err("missing");
        assert!(error.contains("/no/such/rhubarb"), "{error}");
    }
}
