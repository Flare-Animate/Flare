//! Sounds through the MCP server: importing one and reading its waveform frame by frame.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

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

#[tokio::test]
async fn an_imported_sound_reads_back_frame_by_frame_and_an_unknown_one_is_refused() {
    let f = fixture(true, true).await;
    let exports = f.dir.path().join("exports");
    std::fs::create_dir_all(&exports).expect("exports");
    std::fs::write(exports.join("voice.wav"), half_quiet_wav()).expect("wav");
    let added = f
        .ok("import_sound", json!({ "source": "voice.wav", "start": 2 }))
        .await;

    let reply = f.ok("sound_waveform", json!({})).await;
    assert_eq!(reply["frame_rate"], 24.0);
    let sound = &reply["sounds"][0];
    assert_eq!(sound["sound"], added["sound"]);
    assert_eq!(sound["cues"][0]["start"], 2);
    let frames = sound["frames"].as_array().expect("frames");
    assert_eq!(frames.len(), 24);
    assert_eq!(frames[3], json!([0.0, 0.0]), "the quiet half");
    assert_eq!(frames[20], json!([-0.5, 0.5]), "the loud half");

    let one = f.ok("sound_waveform", json!({ "sound": "voice" })).await;
    assert_eq!(one["sounds"].as_array().map(Vec::len), Some(1));
    let (is_error, message) = f.tool("sound_waveform", json!({ "sound": "music" })).await;
    assert!(is_error, "{message}");
    assert!(message.to_string().contains("music"), "{message}");
}

#[tokio::test]
async fn reading_a_waveform_needs_no_write_access() {
    let f = fixture(false, false).await;
    let reply = f.ok("sound_waveform", json!({})).await;
    assert_eq!(reply["sounds"], json!([]));
}

#[tokio::test]
async fn insert_frame_on_a_layer_with_no_frames_makes_blank_frames_to_it() {
    let f = fixture(true, false).await;
    let layer = f.ok("add_layer", json!({ "name": "Empty" })).await["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    f.ok(
        "frame_operation",
        json!({ "operation": "insert_frame", "at": 9, "layer": layer }),
    )
    .await;
    let info = f.ok("get_info", json!({})).await;
    assert_eq!(
        info["scenes"][0]["frames"], 10,
        "the scene runs to frame 10"
    );
}
