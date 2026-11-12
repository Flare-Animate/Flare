//! Helpers for the tests that import and play sounds.

use super::*;

/// A one-second mono 16-bit WAV file at 8 kHz, written to `dir`.
pub(super) fn wav_file(dir: &std::path::Path, name: &str) -> PathBuf {
    let samples: Vec<i16> = (0..8000).map(|i| ((i % 100) as i16 - 50) * 300).collect();
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::new();
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16_u32.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&1_u16.to_le_bytes());
    out.extend_from_slice(&8000_u32.to_le_bytes());
    out.extend_from_slice(&16_000_u32.to_le_bytes());
    out.extend_from_slice(&2_u16.to_le_bytes());
    out.extend_from_slice(&16_u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample.to_le_bytes());
    }
    let path = dir.join(name);
    std::fs::write(&path, out).expect("write wav");
    path
}

pub(super) fn drain_audio(ws: &mut Workspace) -> Vec<String> {
    ws.take_audio_events()
        .into_iter()
        .map(|e| match e {
            AudioEvent::Stop => "stop".to_owned(),
            AudioEvent::Play { delay_seconds, .. } => format!("play@{delay_seconds:.3}"),
        })
        .collect()
}
