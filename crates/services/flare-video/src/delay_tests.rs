//! The encoder's delay and where each picture lands.

use super::*;

#[test]
fn the_encoders_delay_is_what_the_picture_is_held_back_by() {
    use oxideav_aac::encoder::{EncoderConfig, StreamEncoder};
    // A click in the middle of silence has one place it can be, unlike a steady tone.
    let click_at = 20_000usize;
    let mut pcm = vec![0i16; 48_000 * 2];
    for channel in 0..2 {
        for k in 0..6 {
            pcm[(click_at + k) * 2 + channel] = 30_000;
        }
    }
    let mut encoder = StreamEncoder::new(EncoderConfig {
        sample_rate: audio::RATE,
        channels: 2,
        bitrate: 160_000,
    })
    .expect("encoder");
    let mut adts = Vec::new();
    for chunk in pcm.chunks(audio::FRAME_SAMPLES * 2) {
        adts.extend(encoder.encode_frame(chunk).expect("frame"));
    }
    adts.extend(encoder.finish().expect("tail"));
    let frames = oxideav_aac::decode::StreamDecoder::new()
        .decode_all(&adts)
        .expect("decodes");
    let left: Vec<i16> = frames
        .iter()
        .flat_map(|f| f.pcm.iter().step_by(2).copied())
        .collect();
    let peak = left
        .iter()
        .enumerate()
        .max_by_key(|(_, s)| s.unsigned_abs())
        .map(|(i, _)| i)
        .expect("samples");
    let delay = peak as i64 - click_at as i64;
    assert!(
        (delay - audio::ENCODER_DELAY as i64).abs() <= 6,
        "the click came out {delay} samples late, not {}",
        audio::ENCODER_DELAY
    );
    assert!((audio::DELAY_SECONDS - 1024.0 / 48_000.0).abs() < 1e-12);
}

/// With ffprobe on the machine: when each picture of the file is shown, in seconds.
fn picture_times(path: &std::path::Path) -> Option<Vec<f64>> {
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v",
            "-show_entries",
            "packet=pts_time",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .ok()
        .filter(|o| o.status.success())?;
    let mut times: Vec<f64> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().trim_end_matches(',').parse().ok())
        .collect();
    times.sort_by(f64::total_cmp);
    Some(times)
}

#[test]
fn with_sound_every_picture_after_the_first_is_one_encoder_delay_late_and_without_it_none_is() {
    let dir = tempfile::tempdir().expect("dir");
    let loud = dir.path().join("loud.mp4");
    write_mp4(&with_sound(48, 0, tone_wav(4000)), &loud).expect("with sound");
    let silent = dir.path().join("silent.mp4");
    write_mp4(&animation(48, (64, 48)), &silent).expect("silent");
    let Some(with_sound) = picture_times(&loud) else {
        eprintln!("no ffprobe here: the picture times were not checked");
        return;
    };
    let plain = picture_times(&silent).expect("probe");
    assert_eq!(
        (with_sound.len(), plain.len()),
        (48, 48),
        "no frame is added or lost"
    );
    assert!(
        with_sound[0].abs() < 1e-3 && plain[0].abs() < 1e-3,
        "both start at zero"
    );
    for n in 1..48 {
        let on_time = n as f64 / 24.0;
        assert!(
            (plain[n] - on_time).abs() < 1e-3,
            "without sound {n}: {}",
            plain[n]
        );
        assert!(
            (with_sound[n] - on_time - audio::DELAY_SECONDS).abs() < 1e-3,
            "with sound {n}: {} against {}",
            with_sound[n],
            on_time + audio::DELAY_SECONDS
        );
    }
}
