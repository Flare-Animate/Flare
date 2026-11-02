//! Audio for Flare: decode library sounds (WAV, MP3, OGG Vorbis,
//! FLAC via Symphonia), mix the timeline's keyframe sounds the way Animate
//! plays them (Event / Start / Stop / Stream, repeat or loop), draw
//! waveforms, guess mouth shapes for Auto Lip-Sync, and (feature
//! `playback`) play through the system output.

use ac_core::model::Document;
use ac_core::sound::{SoundAsset, SoundId, SoundSync, Viseme};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(feature = "playback")]
#[path = "ac_audio_playback.rs"]
pub mod playback;

/// Decoded audio: interleaved stereo f32 at `rate`.
#[derive(Debug)]
pub struct Pcm {
    pub rate: u32,
    pub samples: Vec<f32>,
}

impl Pcm {
    pub fn frames(&self) -> usize {
        self.samples.len() / 2
    }
    pub fn seconds(&self) -> f32 {
        self.frames() as f32 / self.rate.max(1) as f32
    }
    /// Stereo sample pair at (fractional) frame `i`, linear interpolation.
    fn at(&self, i: f64) -> (f32, f32) {
        let n = self.frames();
        if n == 0 || i < 0.0 {
            return (0.0, 0.0);
        }
        let i0 = i.floor() as usize;
        if i0 >= n {
            return (0.0, 0.0);
        }
        let t = (i - i0 as f64) as f32;
        let i1 = (i0 + 1).min(n - 1);
        let (a, b) = ((self.samples[i0 * 2], self.samples[i0 * 2 + 1]), (self.samples[i1 * 2], self.samples[i1 * 2 + 1]));
        (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
    }
}

/// Decode an audio file to stereo f32.
pub fn decode(bytes: &[u8]) -> Result<Pcm, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::errors::Error;
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    let mss = MediaSourceStream::new(Box::new(std::io::Cursor::new(bytes.to_vec())), Default::default());
    let probed = symphonia::default::get_probe().format(&Hint::new(), mss, &FormatOptions::default(), &MetadataOptions::default()).map_err(|e| format!("unsupported audio file ({e})"))?;
    let mut format = probed.format;
    let track = format.tracks().iter().find(|t| t.codec_params.codec != CODEC_TYPE_NULL).ok_or("no audio track")?;
    let tid = track.id;
    let mut rate = track.codec_params.sample_rate.unwrap_or(44100);
    let mut dec = symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default()).map_err(|e| format!("unsupported codec ({e})"))?;
    let mut out = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(Error::IoError(_)) | Err(Error::ResetRequired) => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id() != tid {
            continue;
        }
        match dec.decode(&packet) {
            Ok(buf) => {
                let spec = *buf.spec();
                rate = spec.rate;
                let ch = spec.channels.count().max(1);
                let mut sb = SampleBuffer::<f32>::new(buf.capacity() as u64, spec);
                sb.copy_interleaved_ref(buf);
                for fr in sb.samples().chunks(ch) {
                    let l = fr[0];
                    let r = if ch > 1 { fr[1] } else { l };
                    out.extend_from_slice(&[l, r]);
                }
            }
            Err(Error::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    if out.is_empty() {
        return Err("the file holds no audio".into());
    }
    Ok(Pcm { rate, samples: out })
}

/// Decoded audio of a library sound (cached).
pub fn pcm(s: &SoundAsset) -> Option<Arc<Pcm>> {
    type Cache = Mutex<HashMap<(SoundId, usize, u64), Option<Arc<Pcm>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let mut h: u64 = 1469598103934665603;
    for x in s.data.iter().step_by((s.data.len() / 4096).max(1)) {
        h = (h ^ *x as u64).wrapping_mul(1099511628211);
    }
    let key = (s.id, s.data.len(), h);
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(v) = cache.lock().unwrap().get(&key) {
        return v.clone();
    }
    let v = decode(&s.data).ok().map(Arc::new);
    cache.lock().unwrap().insert(key, v.clone());
    v
}

/// Min/max of each of `buckets` slices of `[t0, t1)` seconds (mono).
pub fn peaks(p: &Pcm, t0: f32, t1: f32, buckets: usize) -> Vec<(f32, f32)> {
    let n = p.frames();
    let (a, b) = ((t0.max(0.0) * p.rate as f32) as usize, ((t1 * p.rate as f32) as usize).min(n));
    (0..buckets)
        .map(|i| {
            let s = a + (b.saturating_sub(a)) * i / buckets;
            let e = (a + (b.saturating_sub(a)) * (i + 1) / buckets).max(s + 1).min(n);
            let mut lo = 0f32;
            let mut hi = 0f32;
            for k in s..e {
                let v = (p.samples[k * 2] + p.samples[k * 2 + 1]) * 0.5;
                lo = lo.min(v);
                hi = hi.max(v);
            }
            (lo, hi)
        })
        .collect()
}

/// One sound playing in the mix (times in seconds from the movie start).
#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    pub sound: SoundId,
    pub start: f64,
    /// Cut here (Stream span end, a Stop keyframe, or the movie's end).
    pub end: f64,
    pub sync: SoundSync,
    pub effect: ac_core::sound::SoundEffect,
    /// Planned length before any cut (for fades), seconds; infinite = loop.
    pub planned: f64,
    /// Where in the sound it starts (the keyframe's cut) and how much of it plays per
    /// repeat, seconds.
    pub offset: f64,
    pub seg: f64,
}

/// The sounds the movie plays, in Animate's semantics, for linear playback
/// of all scenes (main timeline and symbols). `length` gives a sound's
/// duration in seconds.
pub fn schedule(doc: &Document, length: &dyn Fn(SoundId) -> Option<f64>) -> Vec<Voice> {
    let fps = doc.fps.max(1.0) as f64;
    let frames = doc.total_frames();
    let movie_end = frames as f64 / fps;
    let mut voices: Vec<Voice> = vec![];
    if doc.sounds.is_empty() {
        return voices;
    }
    for f in 0..frames {
        let t = f as f64 / fps;
        for cue in ac_core::sound::frame_sounds(doc, f) {
            let fs = cue.sound;
            let Some(len) = length(fs.sound) else { continue };
            // the cut made in the sound (Animate's in/out points): only that part plays, and repeats
            let (offset, seg) = fs.segment(len);
            let total = if fs.forever { f64::INFINITY } else { seg * fs.repeat.max(1) as f64 };
            let playing = |v: &Voice| v.sound == fs.sound && v.start <= t && v.end > t;
            match fs.sync {
                SoundSync::Stop => {
                    for v in voices.iter_mut().filter(|v| playing(v)) {
                        v.end = t;
                    }
                }
                SoundSync::Start if voices.iter().any(playing) => {}
                SoundSync::Event | SoundSync::Start => voices.push(Voice { sound: fs.sound, start: t, end: (t + total).min(movie_end), sync: fs.sync, effect: fs.effect, planned: total, offset, seg }),
                SoundSync::Stream => {
                    let span_end = cue.stream_end as f64 / fps;
                    voices.push(Voice { sound: fs.sound, start: t, end: (t + total).min(span_end).min(movie_end), sync: fs.sync, effect: fs.effect, planned: total.min(span_end - t), offset, seg })
                }
            }
        }
    }
    voices.retain(|v| v.end > v.start);
    voices
}

/// Mix the movie's sound from frame `from` to its end at `rate` Hz
/// (interleaved stereo). Like Animate, starting mid-movie only joins Stream
/// sounds already under way; Event sounds start at their own keyframes.
pub fn mix(doc: &Document, from: u32, rate: u32) -> Vec<f32> {
    let pcms: HashMap<SoundId, Arc<Pcm>> = doc.sounds.iter().filter_map(|s| pcm(s).map(|p| (s.id, p))).collect();
    let voices = schedule(doc, &|id| pcms.get(&id).map(|p| p.seconds() as f64));
    let fps = doc.fps.max(1.0) as f64;
    let t0 = from as f64 / fps;
    let t1 = doc.total_frames() as f64 / fps;
    let n = ((t1 - t0).max(0.0) * rate as f64).round() as usize;
    let mut out = vec![0f32; n * 2];
    for v in voices {
        if v.end <= t0 || (v.start < t0 && v.sync != SoundSync::Stream) {
            continue;
        }
        let p = &pcms[&v.sound];
        let len = v.seg;
        let a = ((v.start.max(t0) - t0) * rate as f64) as usize;
        let b = (((v.end - t0) * rate as f64) as usize).min(n);
        for i in a..b {
            let t = t0 + i as f64 / rate as f64 - v.start;
            let local = v.offset + if len > 0.0 { t % len } else { 0.0 };
            let (l, r) = p.at(local * p.rate as f64);
            let (gl, gr) = v.effect.gains(t, len, v.planned);
            out[i * 2] += l * gl;
            out[i * 2 + 1] += r * gr;
        }
    }
    for s in &mut out {
        *s = s.clamp(-1.0, 1.0);
    }
    out
}

/// True when any keyframe of the movie starts a sound.
pub fn has_sound(doc: &Document) -> bool {
    !doc.sounds.is_empty() && doc.scenes().flat_map(|t| &t.layers).flat_map(|l| l.keyframes.values()).any(|k| k.sound.is_some())
}

/// 16-bit PCM WAV file bytes.
pub fn wav(samples: &[f32], rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes());
    v.extend_from_slice(&2u16.to_le_bytes());
    v.extend_from_slice(&rate.to_le_bytes());
    v.extend_from_slice(&(rate * 4).to_le_bytes());
    v.extend_from_slice(&4u16.to_le_bytes());
    v.extend_from_slice(&16u16.to_le_bytes());
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    v
}

// ------------------------------------------------------------- lip-sync

/// Energy in each of `edges.len()-1` frequency bands (Hz) of `x`: a
/// Hann-windowed radix-2 FFT.
fn band_energy(x: &[f32], rate: f32, edges: &[f32]) -> Vec<f32> {
    let n = x.len().next_power_of_two().clamp(256, 8192);
    let mut re = vec![0f32; n];
    let mut im = vec![0f32; n];
    let m = x.len().min(n);
    for i in 0..m {
        let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / m.max(2) as f32).cos();
        re[i] = x[i] * w;
    }
    // bit reversal
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f32::consts::TAU / len as f32;
        for start in (0..n).step_by(len) {
            for k in 0..len / 2 {
                let (wr, wi) = ((ang * k as f32).cos(), (ang * k as f32).sin());
                let (a, b) = (start + k, start + k + len / 2);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
    let hz = rate / n as f32;
    edges
        .windows(2)
        .map(|e| {
            let (a, b) = ((e[0] / hz) as usize, ((e[1] / hz) as usize).min(n / 2));
            (a..b.max(a)).map(|k| re[k] * re[k] + im[k] * im[k]).sum()
        })
        .collect()
}

/// Guess a mouth shape for each of `n` frames from `t0` seconds into the
/// sound, at `fps`. A heuristic (loudness, zero crossings and a few
/// frequency bands), not speech recognition — good enough to block in
/// timing that you then adjust, like Animate's Auto Lip-Sync.
pub fn visemes(p: &Pcm, t0: f32, fps: f32, n: usize) -> Vec<Viseme> {
    let rate = p.rate as f32;
    let win = (rate / fps.max(1.0)) as usize;
    let feats: Vec<(f32, f32, [f32; 4])> = (0..n)
        .map(|i| {
            let a = ((t0 + i as f32 / fps) * rate) as usize;
            let mono: Vec<f32> = (a..a + win).filter(|&k| k < p.frames()).map(|k| (p.samples[k * 2] + p.samples[k * 2 + 1]) * 0.5).collect();
            if mono.is_empty() {
                return (0.0, 0.0, [0.0; 4]);
            }
            let rms = (mono.iter().map(|v| v * v).sum::<f32>() / mono.len() as f32).sqrt();
            let zc = mono.windows(2).filter(|w| (w[0] >= 0.0) != (w[1] >= 0.0)).count() as f32 / mono.len() as f32;
            let e = band_energy(&mono, rate, &[120.0, 600.0, 1500.0, 3500.0, 9000.0]);
            let bands = [e[0], e[1], e[2], e[3]];
            (rms, zc, bands)
        })
        .collect();
    let peak = feats.iter().map(|f| f.0).fold(0.0, f32::max).max(1e-6);
    let mut out: Vec<Viseme> = feats
        .iter()
        .map(|&(rms, zc, b)| {
            let loud = rms / peak;
            if loud < 0.08 {
                return Viseme::Rest;
            }
            let sum: f32 = b.iter().sum::<f32>().max(1e-12);
            let (low, mid, high, hiss) = (b[0] / sum, b[1] / sum, b[2] / sum, b[3] / sum);
            if zc > 0.25 || hiss > 0.45 {
                Viseme::Fv
            } else if low > 0.6 {
                if loud > 0.5 {
                    Viseme::O
                } else {
                    Viseme::U
                }
            } else if mid > 0.45 {
                if loud > 0.45 {
                    Viseme::Ai
                } else {
                    Viseme::Etc
                }
            } else if high > 0.4 {
                Viseme::E
            } else if low > 0.4 && mid > 0.25 {
                Viseme::Wq
            } else if loud < 0.25 {
                Viseme::L
            } else {
                Viseme::Etc
            }
        })
        .collect();
    // a short silence between sounds is a closed mouth (M, B, P)
    for i in 1..out.len().saturating_sub(1) {
        if out[i] == Viseme::Rest && out[i - 1] != Viseme::Rest && out.get(i + 1).is_some_and(|v| *v != Viseme::Rest) {
            out[i] = Viseme::Mbp;
        }
    }
    // drop one-frame flickers
    for i in 1..out.len().saturating_sub(1) {
        if out[i - 1] == out[i + 1] && out[i] != out[i - 1] && out[i] != Viseme::Mbp {
            out[i] = out[i - 1];
        }
    }
    out
}

#[cfg(test)]
#[path = "ac_audio_tests.rs"]
mod tests;

#[path = "ac_audio_rhubarb.rs"]
pub mod rhubarb;

/// Give ac-core the decoders it needs for Lip-Sync and sound lengths. Lip-Sync
/// uses Rhubarb Lip Sync when it's installed (and preferred), else the
/// built-in heuristic.
pub fn register() {
    ac_core::sound::set_analyzer(
        |s, t0, fps, n| {
            let p = pcm(s)?;
            if rhubarb::preferred() {
                if let Some(exe) = rhubarb::find() {
                    if let Ok(v) = rhubarb::visemes(&exe, &p, t0, fps, n) {
                        return Some(v);
                    }
                }
            }
            Some(visemes(&p, t0, fps, n))
        },
        |s| pcm(s).map(|p| p.seconds()),
    );
}
