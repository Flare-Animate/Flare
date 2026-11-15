//! Lip Syncing in the Object tab: a selected mouth Graphic keyed to a sound on the timeline.

use super::*;
use flare_core::{InstanceId, Rgba8, ShapeId, SoundId, Transform2D};
use flare_document::{Instance, PlayMode, Shape, Sound, SoundCue, Symbol, SymbolKind};
use flare_runtime::Analysis;
use flare_tools::lipsync::Viseme;

/// At 10 fps: a ten-drawing mouth Graphic keyed at frame 0 on the first layer, selected, and a
/// voice cued at 0 that is silent for half a second, then hums low for half a second. With
/// `voice` false the timeline has no sound.
fn mouth_workspace(voice: bool) -> Workspace {
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
    let mut layers = vec![Layer::new(mouth_layer, "Mouth", vec![key]).expect("layer")];
    if voice {
        layers.push(
            Layer::new(LayerId::new(), "Voice", vec![])
                .expect("layer")
                .with_cues(vec![SoundCue { start: 0, sound }])
                .expect("cue"),
        );
    }
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(10, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "Scene 1", Timeline::new(layers))],
        vec![mouths],
    )
    .expect("document")
    .with_sounds(vec![
        Sound::new(sound, "voice", wav, rate, 1, u64::from(rate)).expect("sound"),
    ])
    .expect("sounds");
    let mut ws = Workspace::new(document, None);
    ws.selected = Some(SpanRef {
        layer: mouth_layer,
        start: 0,
        element: 0,
    });
    ws
}

fn drawings(ws: &Workspace) -> Vec<u32> {
    ws.document().scenes()[0].timeline().layers()[0]
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

fn drawn(ws: &mut Workspace) -> Vec<String> {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut all = Vec::new();
    for step in 0..4 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| ws.show(ui));
        out.textures_delta.clear();
        all = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
    }
    all
}

#[test]
fn syncing_keys_the_mouth_to_the_voice_with_its_mapping_in_one_undo_step() {
    let mut ws = mouth_workspace(true);
    assert_eq!(ws.lip_sync_sounds().len(), 1);
    ws.set_lip_sync_analysis(Analysis::BuiltIn);
    ws.set_lip_sync_map(ws.lip_sync_map().with(Viseme::O, 6));
    ws.start_lip_sync();
    ws.drain_background();
    assert!(!ws.lip_sync_pending());
    assert_eq!(drawings(&ws), [0, 0, 0, 0, 0, 6, 6, 6, 6, 6]);
    assert!(
        ws.status().is_some_and(|s| s.contains("2 keyframes")),
        "{:?}",
        ws.status()
    );
    ws.undo();
    let layer = &ws.document().scenes()[0].timeline().layers()[0];
    assert_eq!(layer.spans().len(), 1, "one step undoes it");
    // The mapping is kept for the symbol, so syncing again needs no setting up.
    assert_eq!(ws.lip_sync_map().frame(Viseme::O), 6);
}

#[test]
fn a_result_for_a_document_changed_since_is_not_applied() {
    let mut ws = mouth_workspace(true);
    ws.set_lip_sync_analysis(Analysis::BuiltIn);
    ws.start_lip_sync();
    let scene = ws.scene_id().expect("scene");
    // An edit lands before the worker's answer is taken.
    ws.run_batch(
        vec![Command::AddLayer {
            scene,
            layer: Layer::new(LayerId::new(), "Later", vec![]).expect("layer"),
        }],
        "add a layer",
    );
    ws.drain_background();
    assert!(
        ws.status().is_some_and(|s| s.contains("changed")),
        "{:?}",
        ws.status()
    );
    let layer = &ws.document().scenes()[0].timeline().layers()[0];
    assert_eq!(layer.spans().len(), 1, "nothing was keyed");
}

#[test]
fn the_section_shows_for_a_selected_graphic_and_asks_for_a_sound_when_there_is_none() {
    let mut ws = mouth_workspace(true);
    let text = drawn(&mut ws);
    for wanted in [
        "Lip Syncing",
        "voice",
        "Automatic",
        "Neutral",
        "MBP",
        "Sync to Sound",
    ] {
        assert!(text.iter().any(|t| t == wanted), "{wanted}: {text:?}");
    }
    let mut ws = mouth_workspace(false);
    let text = drawn(&mut ws);
    assert!(text.iter().any(|t| t == "Lip Syncing"), "{text:?}");
    assert!(
        text.iter()
            .any(|t| t.starts_with("Put a sound on the timeline")),
        "{text:?}"
    );
    ws.start_lip_sync();
    assert!(ws.status().is_some_and(|s| s.starts_with("Put a sound")));
    assert!(!ws.lip_sync_pending());
}
