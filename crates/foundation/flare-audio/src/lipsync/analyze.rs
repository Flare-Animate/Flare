//! The built-in reading of a voice: each frame's loudness, how often the wave crosses zero (a hiss
//! crosses it all the time) and how its energy falls in four bands, turned into a mouth shape by
//! fixed rules. The rules are the archived Flare's. They get the timing right and the shapes
//! roughly so, which is what a lip sync is adjusted from.

use super::fft::band_energy;
use super::{LipSyncError, Viseme, frames_for};
use crate::Audio;

/// The band edges, in Hz: the voice's body, the first formant, the second, and the hiss of F, S
/// and SH.
const BANDS: [f32; 5] = [120.0, 600.0, 1500.0, 3500.0, 9000.0];
/// Quieter than this share of the loudest frame is silence.
const SILENT: f32 = 0.08;
/// Zero crossings a second above which a frame is a hiss (a wave whose main pitch is above about
/// 5.5 kHz). The archive counted crossings per sample, which made a vowel a hiss in a sound
/// recorded at a low sample rate; per second, the rule is the same at every rate.
const HISS_CROSSINGS: f32 = 11_000.0;

struct Features {
    rms: f32,
    /// Zero crossings a second.
    crossings: f32,
    /// Each band's share of the energy of all four.
    bands: [f32; 4],
}

/// One mouth shape for each frame of `audio` at `per_second` frames a second, from its start to
/// its end (the last frame partly covered counts).
pub fn analyze(audio: &Audio, per_second: f64) -> Result<Vec<Viseme>, LipSyncError> {
    let frames = frames_for(audio.seconds(), per_second)?;
    let rate = audio.sample_rate() as f32;
    let channels = usize::from(audio.channels());
    let features: Vec<Features> = (0..frames)
        .map(|frame| {
            let from = sample_at(frame, per_second, audio.sample_rate()).min(audio.frames());
            let to = sample_at(frame + 1, per_second, audio.sample_rate()).min(audio.frames());
            let mono: Vec<f32> = (from..to)
                .map(|at| {
                    let all = &audio.samples()[at * channels..(at + 1) * channels];
                    all.iter().sum::<f32>() / channels as f32
                })
                .collect();
            features_of(&mono, rate)
        })
        .collect();
    let loudest = features.iter().map(|f| f.rms).fold(0.0, f32::max);
    let mut shapes: Vec<Viseme> = features
        .iter()
        .map(|f| shape_of(f, f.rms / loudest.max(1e-6)))
        .collect();
    close_short_gaps(&mut shapes);
    drop_flickers(&mut shapes);
    Ok(shapes)
}

fn sample_at(frame: usize, per_second: f64, rate: u32) -> usize {
    (frame as f64 * f64::from(rate) / per_second) as usize
}

fn features_of(mono: &[f32], rate: f32) -> Features {
    if mono.is_empty() {
        return Features {
            rms: 0.0,
            crossings: 0.0,
            bands: [0.0; 4],
        };
    }
    let len = mono.len() as f32;
    let rms = (mono.iter().map(|v| v * v).sum::<f32>() / len).sqrt();
    let crossings = mono
        .windows(2)
        .filter(|pair| (pair[0] >= 0.0) != (pair[1] >= 0.0))
        .count() as f32
        * rate
        / len;
    let energy = band_energy(mono, rate, &BANDS);
    let total = energy.iter().sum::<f32>().max(1e-12);
    let mut bands = [0.0; 4];
    for (share, e) in bands.iter_mut().zip(&energy) {
        *share = e / total;
    }
    Features {
        rms,
        crossings,
        bands,
    }
}

fn shape_of(f: &Features, loud: f32) -> Viseme {
    if loud < SILENT {
        return Viseme::Neutral;
    }
    let [low, mid, high, hiss] = f.bands;
    if f.crossings > HISS_CROSSINGS || hiss > 0.45 {
        Viseme::Fv
    } else if low > 0.6 {
        if loud > 0.5 { Viseme::O } else { Viseme::U }
    } else if mid > 0.45 {
        if loud > 0.45 { Viseme::Ai } else { Viseme::Etc }
    } else if high > 0.4 {
        Viseme::E
    } else if low > 0.4 && mid > 0.25 {
        Viseme::Wq
    } else if loud < 0.25 {
        Viseme::L
    } else {
        Viseme::Etc
    }
}

/// A single silent frame between two sounds is the lips closing on an M, B or P.
fn close_short_gaps(shapes: &mut [Viseme]) {
    for i in 1..shapes.len().saturating_sub(1) {
        if shapes[i] == Viseme::Neutral
            && shapes[i - 1] != Viseme::Neutral
            && shapes[i + 1] != Viseme::Neutral
        {
            shapes[i] = Viseme::Mbp;
        }
    }
}

/// A shape held for one frame between two of another is noise, unless it is a closed mouth.
fn drop_flickers(shapes: &mut [Viseme]) {
    for i in 1..shapes.len().saturating_sub(1) {
        if shapes[i - 1] == shapes[i + 1] && shapes[i] != shapes[i - 1] && shapes[i] != Viseme::Mbp
        {
            shapes[i] = shapes[i - 1];
        }
    }
}
