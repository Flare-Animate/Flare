use super::*;

/// A second at 8 kHz: half a second of silence, then half a second at about 0.5 either way.
fn half_quiet_wav(dir: &Path) -> String {
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
    let file = dir.join("voice.wav");
    std::fs::write(&file, out).expect("write wav");
    file.to_string_lossy().into_owned()
}

#[test]
fn an_imported_sound_reads_back_frame_by_frame_from_its_cue() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let wav = half_quiet_wav(dir.path());
    json(&["new", &file]); // 24 fps
    let added = json(&["sound-import", &file, &wav, "--at", "5"]);
    assert_eq!(added["name"], "voice");

    let reply = json(&["sound-waveform", &file]);
    assert_eq!(reply["frame_rate"], 24.0);
    let sound = &reply["sounds"][0];
    assert_eq!(sound["sound"], added["sound"]);
    assert_eq!(sound["cues"][0]["start"], 5);
    let frames = sound["frames"].as_array().expect("frames");
    assert_eq!(frames.len(), 24);
    assert_eq!(frames[3], serde_json::json!([0.0, 0.0]), "the quiet half");
    assert_eq!(frames[20], serde_json::json!([-0.5, 0.5]), "the loud half");

    let one = json(&["sound-waveform", &file, "--sound", "voice"]);
    assert_eq!(one["sounds"].as_array().map(Vec::len), Some(1));
}

#[test]
fn asking_for_a_sound_the_project_lacks_is_an_error_naming_it() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let none = json(&["sound-waveform", &file]);
    assert_eq!(none["sounds"], serde_json::json!([]));
    let output = run(&["sound-waveform", &file, "--sound", "music"]);
    assert!(!output.status.success());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(message.contains("music"), "{message}");
}

#[test]
fn insert_frame_on_a_layer_with_no_frames_makes_blank_frames_to_it() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let layer = json(&["add-layer", &file, "Empty"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&[
        "frame",
        "insert-frame",
        &file,
        "--at",
        "9",
        "--layer",
        &layer,
    ]);
    let info = json(&["info", &file]);
    let empty = info["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .find(|l| l["id"] == layer.as_str())
        .expect("the layer");
    assert_eq!(empty["spans"], 1, "one run of blank frames");
}
