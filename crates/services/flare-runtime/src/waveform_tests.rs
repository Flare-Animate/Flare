#![allow(clippy::expect_used)]

use super::*;
use flare_commands::Command;
use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Layer, Scene, Timeline};
use flare_history::EditorSession;

/// A second at 8 kHz: half a second of silence, then half a second at about 0.5 either way.
fn half_quiet_wav() -> Vec<u8> {
    let samples: Vec<i16> = (0..8000)
        .map(|i| match (i < 4000, i % 2) {
            (true, _) => 0,
            (false, 0) => 16384,
            (false, _) => -16384,
        })
        .collect();
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&8000_u32.to_le_bytes());
    out.extend_from_slice(&16000_u32.to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    out
}

/// A 10 fps project with the sound cued at frame 3.
fn project() -> Document {
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene_id = SceneId::new();
    let scene = Scene::new(scene_id, "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(10, 1).expect("rate");
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document");
    let made = crate::sound_layer("Voice", half_quiet_wav(), 3).expect("sound");
    let mut session = EditorSession::new(document);
    session
        .apply([
            Command::AddSound(made.sound),
            Command::AddLayer {
                scene: scene_id,
                layer: made.layer,
            },
        ])
        .expect("added");
    session.document().clone()
}

#[test]
fn each_frame_from_the_cue_has_the_sounds_low_and_high() {
    let document = project();
    let reply = sound_waveforms(&document, None).expect("waveforms");
    assert_eq!(reply["frame_rate"], 10.0);
    let sound = &reply["sounds"][0];
    assert_eq!(sound["name"], "Voice");
    assert_eq!(sound["cues"][0]["start"], 3);
    let frames = sound["frames"].as_array().expect("frames");
    assert_eq!(frames.len(), 10, "one second at 10 fps");
    assert_eq!(frames[2], json!([0.0, 0.0]), "the quiet half");
    assert_eq!(frames[7], json!([-0.5, 0.5]), "the loud half");
}

#[test]
fn one_sound_is_chosen_by_name_or_id_and_an_unknown_one_is_refused() {
    let document = project();
    let id = document.sounds()[0].id().as_uuid().to_string();
    for wanted in ["Voice", id.as_str()] {
        let reply = sound_waveforms(&document, Some(wanted)).expect("found");
        assert_eq!(reply["sounds"].as_array().map(Vec::len), Some(1));
    }
    assert_eq!(
        sound_waveforms(&document, Some("Music")),
        Err(WaveformError::UnknownSound("Music".to_owned()))
    );
}

#[test]
fn frame_peaks_follow_the_frame_rate() {
    let document = project();
    let sound = &document.sounds()[0];
    let fine = frame_peaks(sound, 40.0).expect("peaks");
    assert_eq!(fine.len(), 40);
    assert_eq!(fine[19], [0.0, 0.0]);
    assert_eq!(fine[21], [-0.5, 0.5]);
}
