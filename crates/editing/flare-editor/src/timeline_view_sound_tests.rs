#![allow(clippy::expect_used)]

use super::*;
use flare_commands::Command;
use flare_core::{DocumentId, FrameRate, LayerId, SceneId, SoundId};
use flare_document::{Document, Layer, Scene, Sound, SoundCue, Timeline};
use flare_history::EditorSession;

#[test]
fn a_sound_covers_its_length_in_frames_rounded_up_and_never_none() {
    assert_eq!(sound_frames(1.0, 24.0), 24);
    assert_eq!(sound_frames(1.01, 24.0), 25);
    assert_eq!(sound_frames(0.001, 24.0), 1);
    assert_eq!(sound_frames(f64::NAN, 24.0), 1);
    assert_eq!(sound_frames(-2.0, 24.0), 1);
}

#[test]
fn the_timeline_reaches_the_end_of_the_longest_cued_sound() {
    let scene = SceneId::new();
    let layer = Layer::new(LayerId::new(), "Sound", vec![]).expect("layer");
    let layer_id = layer.id();
    let rate = FrameRate::new(10, 1).expect("rate");
    let document = Document::new(
        DocumentId::new(),
        rate,
        vec![Scene::new(scene, "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document");
    let first_timeline = |d: &Document| d.scenes()[0].timeline().clone();
    assert_eq!(sounds_end(&document, &first_timeline(&document)), 0);
    // Two seconds of sound (16 000 frames at 8 kHz), cued at 3 and at 15.
    let id = SoundId::new();
    let sound = Sound::new(id, "s", vec![1], 8000, 1, 16_000).expect("sound");
    let mut session = EditorSession::new(document);
    session
        .apply([
            Command::AddSound(sound),
            Command::SetCues {
                scene,
                layer: layer_id,
                cues: vec![
                    SoundCue {
                        start: 3,
                        sound: id,
                    },
                    SoundCue {
                        start: 15,
                        sound: id,
                    },
                ],
            },
        ])
        .expect("cued");
    let document = session.document();
    assert_eq!(
        sounds_end(document, &first_timeline(document)),
        35,
        "15 + 20 frames"
    );
}
