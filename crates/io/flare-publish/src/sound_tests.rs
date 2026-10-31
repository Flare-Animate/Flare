#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use super::*;
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId};
use flare_document::{Content, FrameSpan, Scene, Sound, SoundCue, Stage, Timeline};

fn wav(samples: usize) -> Vec<u8> {
    let data_len = (samples * 2) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&11025_u32.to_le_bytes());
    out.extend_from_slice(&22050_u32.to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend(std::iter::repeat_n(0_u8, samples * 2));
    out
}

fn two_sounds() -> Document {
    let (a, b) = (SoundId::new(), SoundId::new());
    let layer = flare_document::Layer::new(
        LayerId::new(),
        "L",
        vec![FrameSpan::new(
            0,
            NonZeroU32::MIN.saturating_add(1),
            Content::Empty,
        )],
    )
    .expect("layer")
    .with_cues(vec![
        SoundCue { start: 0, sound: a },
        SoundCue { start: 1, sound: b },
    ])
    .expect("cues");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let stage = Stage::new(10, 10, Rgba8::WHITE).expect("stage");
    let sound = |id, name| Sound::new(id, name, wav(1000), 11025, 1, 1000).expect("sound");
    Document::new(
        DocumentId::new(),
        FrameRate::new(12, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("doc")
    .with_stage(stage)
    .with_sounds(vec![sound(a, "A"), sound(b, "B")])
    .expect("sounds")
}

#[test]
fn the_total_of_all_sounds_is_bounded() {
    let doc = two_sounds();
    // Each sound is 1000 samples at 11025 Hz, so room for one and a half defines only one.
    let out = define(&doc, 1, 1500);
    assert_eq!(out.define.len(), 1);
    assert_eq!(out.next_character, 2);
    assert!(
        out.diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning && d.message.contains("\"B\"")),
        "{:?}",
        out.diagnostics
    );
    let roomy = define(&doc, 1, 2000);
    assert_eq!(roomy.define.len(), 2);
}

#[test]
fn running_out_of_character_ids_skips_the_sound() {
    let out = define(&two_sounds(), swf::CharacterId::MAX, MAX_SAMPLES);
    assert!(out.define.is_empty());
    assert!(!out.diagnostics.is_empty());
}

#[test]
fn rates_round_up_to_a_swf_rate() {
    assert_eq!(rate_for(8000), 11025);
    assert_eq!(rate_for(5512), 5512);
    assert_eq!(rate_for(22050), 22050);
    assert_eq!(rate_for(32000), 44100);
    assert_eq!(rate_for(96000), 44100);
}
