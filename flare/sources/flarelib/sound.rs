//! Sounds: library audio files (kept as their original bytes) and the sound
//! a keyframe starts, as in Animate's Properties ▸ Sound. Decoding, mixing,
//! waveforms and lip-sync analysis live in the `ac-audio` crate.

use serde::{Deserialize, Serialize};

pub type SoundId = u32;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SoundAsset {
    pub id: SoundId,
    pub name: String,
    /// The imported file (WAV, MP3, OGG Vorbis or FLAC), base64 in JSON.
    #[serde(serialize_with = "crate::bitmap::b64_ser", deserialize_with = "crate::bitmap::b64_de")]
    pub data: Vec<u8>,
}

/// Animate's Sync setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SoundSync {
    /// Plays to its end on its own, whatever the timeline does.
    #[default]
    Event,
    /// Like Event, but not if the same sound is already playing.
    Start,
    /// Silences the sound.
    Stop,
    /// Tied to the timeline: plays only while its frames do, cut at the span end.
    Stream,
}

impl SoundSync {
    pub const ALL: [SoundSync; 4] = [SoundSync::Event, SoundSync::Start, SoundSync::Stop, SoundSync::Stream];
    pub fn name(self) -> &'static str {
        match self {
            SoundSync::Event => "Event",
            SoundSync::Start => "Start",
            SoundSync::Stop => "Stop",
            SoundSync::Stream => "Stream",
        }
    }
    pub fn from_name(s: &str) -> Option<SoundSync> {
        SoundSync::ALL.into_iter().find(|x| x.name().eq_ignore_ascii_case(s.trim()))
    }
}

/// Animate's Effect setting (Properties ▸ Sound).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SoundEffect {
    #[default]
    None,
    LeftChannel,
    RightChannel,
    FadeLeftToRight,
    FadeRightToLeft,
    FadeIn,
    FadeOut,
}

impl SoundEffect {
    pub const ALL: [SoundEffect; 7] = [SoundEffect::None, SoundEffect::LeftChannel, SoundEffect::RightChannel, SoundEffect::FadeLeftToRight, SoundEffect::FadeRightToLeft, SoundEffect::FadeIn, SoundEffect::FadeOut];
    pub fn name(self) -> &'static str {
        match self {
            SoundEffect::None => "None",
            SoundEffect::LeftChannel => "Left channel",
            SoundEffect::RightChannel => "Right channel",
            SoundEffect::FadeLeftToRight => "Fade to right",
            SoundEffect::FadeRightToLeft => "Fade to left",
            SoundEffect::FadeIn => "Fade in",
            SoundEffect::FadeOut => "Fade out",
        }
    }
    pub fn from_name(s: &str) -> Option<SoundEffect> {
        let k = s.trim().to_ascii_lowercase().replace(['_', '-'], " ");
        SoundEffect::ALL.into_iter().find(|e| e.name().to_ascii_lowercase() == k || format!("{e:?}").to_ascii_lowercase() == k.replace(' ', ""))
    }
    /// (left, right) gain at `t` seconds into a voice lasting `total`
    /// seconds whose sound is `len` seconds long. Fades run over one play of
    /// the sound (the first for fade in, the last for fade out).
    pub fn gains(self, t: f64, len: f64, total: f64) -> (f32, f32) {
        let len = len.max(1e-6);
        let span = if total.is_finite() { total.max(1e-6) } else { len };
        match self {
            SoundEffect::None => (1.0, 1.0),
            SoundEffect::LeftChannel => (1.0, 0.0),
            SoundEffect::RightChannel => (0.0, 1.0),
            SoundEffect::FadeLeftToRight => {
                let k = (t / span).clamp(0.0, 1.0) as f32;
                (1.0 - k, k)
            }
            SoundEffect::FadeRightToLeft => {
                let k = (t / span).clamp(0.0, 1.0) as f32;
                (k, 1.0 - k)
            }
            SoundEffect::FadeIn => {
                let k = (t / len).clamp(0.0, 1.0) as f32;
                (k, k)
            }
            SoundEffect::FadeOut => {
                if !total.is_finite() {
                    return (1.0, 1.0);
                }
                let k = ((total - t) / len.min(total)).clamp(0.0, 1.0) as f32;
                (k, k)
            }
        }
    }
}

/// The sound a keyframe starts (or stops).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameSound {
    pub sound: SoundId,
    #[serde(default)]
    pub sync: SoundSync,
    /// Repeat count (1 = play once); ignored when `forever`.
    #[serde(default = "one")]
    pub repeat: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub forever: bool,
    #[serde(default, skip_serializing_if = "is_no_effect")]
    pub effect: SoundEffect,
    /// Where the sound was cut to start (Animate's `inPoint44`): samples at 44.1 kHz from
    /// the start of the file, whatever the file's own rate.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub in_point: u32,
    /// Where it was cut to end (`outPoint44`), same units; 0 = the end of the sound.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub out_point: u32,
}

fn is_no_effect(e: &SoundEffect) -> bool {
    *e == SoundEffect::None
}

fn is_zero(n: &u32) -> bool {
    *n == 0
}

fn one() -> u32 {
    1
}

impl FrameSound {
    pub fn new(sound: SoundId) -> Self {
        FrameSound { sound, sync: SoundSync::Event, repeat: 1, forever: false, effect: SoundEffect::None, in_point: 0, out_point: 0 }
    }

    /// The cut in seconds: (where it starts, how long it plays; 0 = to the end of the sound).
    /// An end at or before the start is ignored.
    pub fn trim_seconds(&self) -> (f64, f64) {
        const RATE: f64 = 44_100.0;
        let a = self.in_point as f64 / RATE;
        if self.out_point == 0 {
            (a, 0.0)
        } else if self.out_point > self.in_point {
            (a, (self.out_point - self.in_point) as f64 / RATE)
        } else {
            (0.0, 0.0)
        }
    }

    /// The part of a `len`-second sound this keyframe plays: (offset, length), seconds.
    /// A cut that doesn't fit the sound plays all of it.
    pub fn segment(&self, len: f64) -> (f64, f64) {
        let (a, d) = self.trim_seconds();
        let a = a.min(len);
        let seg = if d > 0.0 { d.min(len - a) } else { len - a };
        if seg > 0.0 { (a, seg) } else { (0.0, len) }
    }
}

/// Mouth shapes for Auto Lip-Sync (Animate's viseme set, simplified).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Viseme {
    Rest,
    Mbp,
    Ai,
    E,
    O,
    U,
    Fv,
    L,
    Wq,
    Etc,
}

impl Viseme {
    pub const ALL: [Viseme; 10] = [Viseme::Rest, Viseme::Mbp, Viseme::Ai, Viseme::E, Viseme::O, Viseme::U, Viseme::Fv, Viseme::L, Viseme::Wq, Viseme::Etc];
    pub fn name(self) -> &'static str {
        match self {
            Viseme::Rest => "Neutral",
            Viseme::Mbp => "MBP",
            Viseme::Ai => "Ai",
            Viseme::E => "E",
            Viseme::O => "O",
            Viseme::U => "U",
            Viseme::Fv => "FV",
            Viseme::L => "L",
            Viseme::Wq => "WQ",
            Viseme::Etc => "etc",
        }
    }
    pub fn from_name(s: &str) -> Option<Viseme> {
        Viseme::ALL.into_iter().find(|v| v.name().eq_ignore_ascii_case(s.trim()))
    }
}

/// Mouth-shape analysis is provided by the audio crate at startup (ac-core
/// stays free of decoders): (sound, start seconds, fps, frames) → shapes.
pub type Analyzer = fn(&SoundAsset, f32, f32, usize) -> Option<Vec<Viseme>>;
static ANALYZER: std::sync::OnceLock<Analyzer> = std::sync::OnceLock::new();
/// Sound length in seconds, also from the audio crate.
pub type Measurer = fn(&SoundAsset) -> Option<f32>;
static MEASURER: std::sync::OnceLock<Measurer> = std::sync::OnceLock::new();

pub fn set_analyzer(a: Analyzer, m: Measurer) {
    let _ = ANALYZER.set(a);
    let _ = MEASURER.set(m);
}
pub fn analyze(s: &SoundAsset, t0: f32, fps: f32, n: usize) -> Option<Vec<Viseme>> {
    ANALYZER.get().and_then(|a| a(s, t0, fps, n))
}
/// True once the audio crate has registered its decoders.
pub fn has_decoder() -> bool {
    MEASURER.get().is_some()
}
pub fn seconds(s: &SoundAsset) -> Option<f32> {
    MEASURER.get().and_then(|m| m(s))
}

// ---------------------------------------------------------------- sounds in the movie

/// A keyframe sound reached at a movie frame, on the main timeline or
/// inside a symbol.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundCue {
    pub sound: FrameSound,
    /// Movie frame (exclusive) where a Stream sound's frames end.
    pub stream_end: u32,
}

/// (path to the keyframe, symbol-local frame it is at, sound, frames left in its span)
type Hit = (Vec<u32>, u32, FrameSound, u32);

fn collect(doc: &crate::model::Document, tl: &crate::model::Timeline, frame: u32, global: u32, path: &mut Vec<u32>, depth: u32, out: &mut Vec<Hit>) {
    use crate::model::{ElementKind, LayerKind};
    if depth > 16 {
        return;
    }
    for (li, l) in tl.layers.iter().enumerate() {
        if matches!(l.kind, LayerKind::Guide | LayerKind::Folder | LayerKind::Camera) {
            continue;
        }
        let Some((start, kf, _)) = l.span_at(frame) else { continue };
        path.push(li as u32);
        path.push(start);
        if start == frame {
            if let Some(fs) = &kf.sound {
                out.push((path.clone(), frame, fs.clone(), l.span_end(start) - frame));
            }
        }
        fn walk(doc: &crate::model::Document, els: &[crate::model::Element], local: u32, global: u32, path: &mut Vec<u32>, depth: u32, out: &mut Vec<Hit>) {
            for (ei, el) in els.iter().enumerate() {
                path.push(ei as u32);
                match &el.kind {
                    ElementKind::Instance(inst) => {
                        if let Some(sym) = doc.symbol(inst.symbol) {
                            let dur = sym.timeline.frame_count();
                            let f = inst.symbol_frame(sym.kind, local, global, dur);
                            path.push(inst.symbol);
                            collect(doc, &sym.timeline, f, global, path, depth + 1, out);
                            path.pop();
                        }
                    }
                    ElementKind::Group(ch) => walk(doc, ch, local, global, path, depth, out),
                    _ => {}
                }
                path.pop();
            }
        }
        walk(doc, &kf.elements, frame - start, global, path, depth, out);
        path.pop();
        path.pop();
    }
}

fn hits_at(doc: &crate::model::Document, global: u32) -> Vec<Hit> {
    let (si, local) = doc.locate_frame(global);
    let Some(tl) = doc.scene_at(si) else { return vec![] };
    let mut out = vec![];
    let mut path = vec![si as u32];
    collect(doc, tl, local, global, &mut path, 0, &mut out);
    out
}

/// Sounds that start (or stop) at movie frame `global`: keyframes entered on
/// the main timeline and inside symbols (a looping graphic replays its sound
/// each time it comes round, as in Animate). A sound counts as entered when
/// its keyframe wasn't showing at the same point on the previous frame.
pub fn frame_sounds(doc: &crate::model::Document, global: u32) -> Vec<SoundCue> {
    if global >= doc.total_frames() {
        return vec![];
    }
    let now = hits_at(doc, global);
    if now.is_empty() {
        return vec![];
    }
    let before = if global == 0 { vec![] } else { hits_at(doc, global - 1) };
    now.into_iter()
        .filter(|(p, f, _, _)| !before.iter().any(|(q, g, _, _)| q == p && g == f))
        .map(|(_, _, sound, left)| SoundCue { sound, stream_end: global + left.max(1) })
        .collect()
}
