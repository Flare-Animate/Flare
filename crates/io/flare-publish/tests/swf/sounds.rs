use super::*;
use flare_core::SoundId;
use flare_document::{Sound, SoundCue};

/// A 16-bit PCM WAV file holding `samples` (interleaved).
fn wav(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    out.extend_from_slice(&(channels * 2).to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

fn sound(name: &str, bytes: Vec<u8>, rate: u32, channels: u16, frames: u64) -> Sound {
    Sound::new(SoundId::new(), name, bytes, rate, channels, frames).expect("sound")
}

/// A document whose one layer cues each `(frame, sound)`.
fn cued(frames: u32, sounds: Vec<Sound>, cues: &[(u32, usize)]) -> Document {
    let cues = cues
        .iter()
        .map(|(start, which)| SoundCue {
            start: *start,
            sound: sounds[*which].id(),
        })
        .collect();
    let shape = rect(0.0, 0.0, 10.0, 10.0, Rgba8::WHITE);
    let layer = layer("L", vec![span(0, frames, Content::Shape(shape))])
        .with_cues(cues)
        .expect("cues");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    document(10, 10, vec![scene], vec![])
        .with_sounds(sounds)
        .expect("sounds")
}

fn published(doc: &Document) -> (Parsed, Vec<flare_publish::Diagnostic>) {
    let out = publish_swf(doc, &PublishOptions::default()).expect("publish");
    (parse(&out.bytes), out.diagnostics)
}

fn definitions(parsed: &Parsed) -> Vec<&swf::Sound<'static>> {
    parsed
        .tags
        .iter()
        .filter_map(|t| match t {
            Tag::DefineSound(s) => Some(s),
            _ => None,
        })
        .collect()
}

/// For each StartSound, the frame it is shown with and the character it starts.
fn starts(parsed: &Parsed) -> Vec<(usize, u16)> {
    let mut frame = 0;
    let mut out = Vec::new();
    for tag in &parsed.tags {
        match tag {
            Tag::ShowFrame => frame += 1,
            Tag::StartSound(s) => out.push((frame, s.id)),
            _ => {}
        }
    }
    out
}

#[test]
fn a_cued_sound_is_defined_once_and_started_at_each_cue() {
    let samples: Vec<i16> = (0..8000).map(|n| ((n % 100) * 100 - 5000) as i16).collect();
    let clap = sound("Clap", wav(44100, 1, &samples), 44100, 1, 8000);
    let doc = cued(10, vec![clap], &[(2, 0), (7, 0)]);
    let (parsed, diagnostics) = published(&doc);

    let defined = definitions(&parsed);
    assert_eq!(defined.len(), 1);
    let s = defined[0];
    assert_eq!(s.format.compression, swf::AudioCompression::Uncompressed);
    assert_eq!(s.format.sample_rate, 44100);
    assert!(!s.format.is_stereo && s.format.is_16_bit);
    assert_eq!(s.num_samples, 8000);
    let decoded: Vec<i16> = s
        .data
        .chunks(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]))
        .collect();
    assert_eq!(decoded, samples);

    assert_eq!(starts(&parsed), vec![(2, s.id), (7, s.id)]);
    assert!(
        diagnostics
            .iter()
            .all(|d| !d.message.contains("silent") && d.severity != Severity::Warning),
        "{diagnostics:?}"
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d.message.contains("uncompressed"))
    );
}

#[test]
fn sounds_are_resampled_up_to_a_swf_rate_and_stereo_is_kept() {
    let frames = 4800;
    let samples: Vec<i16> = (0..frames * 2).map(|n| (n % 50) as i16 * 10).collect();
    let song = sound("Song", wav(48000, 2, &samples), 48000, 2, frames as u64);
    let low = sound("Low", wav(8000, 1, &vec![100; 800]), 8000, 1, 800);
    let doc = cued(4, vec![song, low], &[(0, 0), (1, 1)]);
    let (parsed, _) = published(&doc);
    let defined = definitions(&parsed);
    assert_eq!(defined.len(), 2);
    // 48 kHz has no SWF rate at or above it, so it goes to 44.1 kHz.
    assert_eq!(defined[0].format.sample_rate, 44100);
    assert!(defined[0].format.is_stereo);
    assert_eq!(defined[0].num_samples, 4410);
    assert_eq!(defined[0].data.len(), 4410 * 2 * 2);
    // 8 kHz goes up to 11.025 kHz, never down to 5.5.
    assert_eq!(defined[1].format.sample_rate, 11025);
    assert_eq!(defined[1].num_samples, 1103);
}

#[test]
fn scenes_offset_the_frames_cues_start_at() {
    let beep = sound("Beep", wav(11025, 1, &[1000; 1000]), 11025, 1, 1000);
    let cue = SoundCue {
        start: 1,
        sound: beep.id(),
    };
    let first = one_frame_scene("A", vec![rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)], 3);
    let second_layer = layer(
        "L",
        vec![span(
            0,
            2,
            Content::Shape(rect(0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)),
        )],
    )
    .with_cues(vec![cue])
    .expect("cues");
    let second = Scene::new(SceneId::new(), "B", Timeline::new(vec![second_layer]));
    let doc = document(10, 10, vec![first, second], vec![])
        .with_sounds(vec![beep])
        .expect("sounds");
    let (parsed, _) = published(&doc);
    assert_eq!(starts(&parsed).iter().map(|s| s.0).collect::<Vec<_>>(), [4]);
}

#[test]
fn shapes_take_the_character_ids_after_the_sounds() {
    let beep = sound("Beep", wav(11025, 1, &[1000; 1000]), 11025, 1, 1000);
    let doc = cued(2, vec![beep], &[(0, 0)]);
    let (parsed, _) = published(&doc);
    let sound_id = definitions(&parsed)[0].id;
    let shape_ids: Vec<u16> = parsed
        .tags
        .iter()
        .filter_map(|t| match t {
            Tag::DefineShape(s) => Some(s.id),
            _ => None,
        })
        .collect();
    assert!(!shape_ids.is_empty());
    assert!(shape_ids.iter().all(|id| *id != sound_id));
}

#[test]
fn a_sound_that_cannot_be_decoded_is_skipped_and_reported() {
    let broken = sound("Clap", vec![1, 2, 3], 8000, 1, 1);
    let doc = cued(3, vec![broken], &[(0, 0)]);
    let (parsed, diagnostics) = published(&doc);
    assert!(definitions(&parsed).is_empty());
    assert!(starts(&parsed).is_empty());
    assert!(
        diagnostics
            .iter()
            .any(|d| d.severity == Severity::Warning && d.message.contains("Clap")),
        "{diagnostics:?}"
    );
}

#[test]
fn a_sound_no_layer_cues_is_not_written() {
    let idle = sound("Idle", wav(11025, 1, &[1000; 100]), 11025, 1, 100);
    let doc = cued(2, vec![idle], &[]);
    let (parsed, diagnostics) = published(&doc);
    assert!(definitions(&parsed).is_empty());
    assert!(
        diagnostics
            .iter()
            .all(|d| !d.message.contains("uncompressed"))
    );
}
