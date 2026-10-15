//! Sounds: library items holding a WAV or MP3 file (XFL's `<DOMSoundItem>`s, in
//! DOMDocument's `<media>`), the sound a keyframe plays (kept as its DOMFrame's
//! attributes, like its label), the rules playing them that the SWF export
//! (sound_swf.rs) and the editor share, and decoding.

use std::fmt;
use std::sync::Arc;

use crate::Result;
use crate::doc::{Document, Extra, Frame, LayerKind, Media, Timeline};

/// A library sound: its audio as its file holds it, and the rest of its XFL item.
#[derive(Clone, PartialEq)]
pub struct Sound {
    /// A WAV or MP3 file. Animate's own copy in `bin/` may instead be bare samples,
    /// laid out as the item's `format` says.
    // ponytail: that layout of bin/*.dat (and MP3s there kept as they are) is unconfirmed, so the
    // imported file in LIBRARY/ is read first; bare samples go back to bin/ as they were read.
    pub data: Arc<[u8]>,
    /// `<DOMSoundItem>`'s attributes but its name: `href` (its file under LIBRARY/),
    /// `soundDataHRef` (Animate's copy under bin/), `format`, `sampleCount`, linkage...
    pub extra: Extra,
}

impl fmt::Debug for Sound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Sound").field("data", &format_args!("{} bytes", self.data.len())).field("extra", &self.extra).finish()
    }
}

/// How a sound's bytes hold its audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Wav,
    Mp3,
    /// Bare samples, as `format` says.
    Raw,
}

/// What a sound holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Info {
    pub codec: Codec,
    pub rate: u32,
    pub channels: u16,
    /// Bits a sample as stored; an MP3 decodes to 16.
    pub bits: u16,
    /// Samples a channel.
    pub samples: u64,
}

impl Info {
    pub fn seconds(&self) -> f64 {
        self.samples as f64 / self.rate.max(1) as f64
    }

    /// As XFL's `format` attribute puts it: "44kHz 16bit Stereo".
    pub fn format(&self) -> String {
        format!("{}kHz {}bit {}", self.rate / 1000, self.bits, if self.channels > 1 { "Stereo" } else { "Mono" })
    }
}

/// How the SWF export codes a sound: Animate's Sound Properties > Compression, kept in
/// the item's `compressionType` and `bits` (JSFL's names for them).
// ponytail: Animate's own attributes for this (exportFormat, exportBits...) aren't read, as
// their values aren't publicly known; they round-trip as they were. No Sound Properties
// dialog sets these yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    /// MP3s go out as they are, everything else as 4-bit ADPCM.
    Default,
    /// ADPCM at 2 to 5 bits a sample.
    Adpcm(u8),
    /// As Default: an MP3 goes as it is, and with no MP3 encoder the rest as ADPCM.
    Mp3,
    /// Uncompressed 16-bit samples.
    Raw,
}

/// Interleaved 16-bit samples.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

impl Pcm {
    /// Samples a channel.
    pub fn len(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Sample `i` of channel `c` (0 left, 1 right); a mono sound gives its one for both.
    pub fn at(&self, i: usize, c: usize) -> i16 {
        let n = self.channels.max(1) as usize;
        self.samples.get(i * n + c.min(n - 1)).copied().unwrap_or(0)
    }

    /// Resampled to `rate`, straight lines between the samples.
    pub fn resampled(&self, rate: u32) -> Pcm {
        if rate == self.rate || self.is_empty() || self.rate == 0 {
            return Pcm { rate, ..self.clone() };
        }
        let (n, len) = (self.channels.max(1) as usize, self.len());
        let out_len = (len as u64 * rate as u64 / self.rate as u64) as usize;
        let mut samples = Vec::with_capacity(out_len * n);
        for j in 0..out_len {
            let x = j as f64 * self.rate as f64 / rate as f64;
            let (i, t) = (x as usize, x.fract());
            for c in 0..n {
                let (a, b) = (self.at(i, c) as f64, self.at((i + 1).min(len - 1), c) as f64);
                samples.push((a + (b - a) * t).round() as i16);
            }
        }
        Pcm { rate, channels: self.channels, samples }
    }
}

impl Sound {
    /// A library sound for the bytes of a WAV or MP3 file, kept as `href` under LIBRARY/.
    // ponytail: saved as that file, with no bin/ copy (soundDataHRef), whose layout is
    // unconfirmed; that Animate imports such an item from LIBRARY/ is unconfirmed too.
    pub fn from_file(data: Vec<u8>, href: &str) -> Result<Sound> {
        let mut sound = Sound { data: data.into(), extra: Extra::default() };
        let info = sound.info().filter(|i| i.codec != Codec::Raw).ok_or("not a WAV or MP3 file this can play")?;
        sound.extra.set("href", Some(&safe(href).ok_or("no file name")?));
        sound.extra.set("format", Some(&info.format()));
        sound.extra.set("sampleCount", Some(&info.samples.to_string()));
        Ok(sound)
    }

    /// What it holds; None if it isn't audio this can play.
    pub fn info(&self) -> Option<Info> {
        if let Some((tag, channels, rate, bits, body)) = wav(&self.data) {
            if !matches!((tag, bits), (1, 8 | 16 | 24 | 32) | (3, 32 | 64)) || channels == 0 || rate == 0 {
                return None;
            }
            let samples = body.len() as u64 / (channels as u64 * bits as u64 / 8);
            return Some(Info { codec: Codec::Wav, rate, channels, bits, samples });
        }
        if nanomp3::detect(&self.data) {
            let mut frames = mp3_frames(&self.data).peekable();
            let first = frames.peek()?;
            let (rate, channels) = (first.sample_rate, first.channels.num() as u16);
            let samples = frames.map(|f| f.samples as u64).sum();
            return Some(Info { codec: Codec::Mp3, rate, channels, bits: 16, samples });
        }
        let (rate, bits, channels) = raw_format(self.extra.get("format")?)?;
        let samples = self.data.len() as u64 / (channels as u64 * bits as u64 / 8);
        Some(Info { codec: Codec::Raw, rate, channels, bits, samples })
    }

    /// The audio as 16-bit samples.
    pub fn decode(&self) -> Result<Pcm> {
        let info = self.info().ok_or("not audio this can play")?;
        let pcm = match info.codec {
            Codec::Mp3 => {
                let d = nanomp3::decode_all::<i16>(&self.data);
                let channels = d.channels.ok_or("no MP3 audio")?.num() as u16;
                Pcm { rate: d.sample_rate, channels, samples: d.samples }
            }
            Codec::Wav => {
                let (tag, channels, rate, bits, body) = wav(&self.data).ok_or("not a WAV file")?;
                samples(body, tag == 3, bits, channels, rate)
            }
            // ponytail: 8-bit bare samples read as unsigned, as in WAV and SWF; unconfirmed for bin/.
            Codec::Raw => samples(&self.data, false, info.bits, info.channels, info.rate),
        };
        Ok(pcm)
    }

    pub fn compression(&self) -> Compression {
        let bits = self.extra.get("bits").and_then(|b| b.trim_end_matches("bit").trim().parse().ok()).unwrap_or(4u8);
        match self.extra.get("compressionType") {
            Some("ADPCM") => Compression::Adpcm(bits.clamp(2, 5)),
            Some("MP3") => Compression::Mp3,
            Some("Raw") => Compression::Raw,
            _ => Compression::Default,
        }
    }

    pub fn set_compression(&mut self, c: Compression) {
        let (kind, bits) = match c {
            Compression::Default => (None, None),
            Compression::Adpcm(b) => (Some("ADPCM"), Some(format!("{} bit", b.clamp(2, 5)))),
            Compression::Mp3 => (Some("MP3"), None),
            Compression::Raw => (Some("Raw"), None),
        };
        self.extra.set("compressionType", kind);
        self.extra.set("bits", bits.as_deref());
    }
}

/// A WAV file's format tag (WAVE_FORMAT_EXTENSIBLE's sub-format for that one), channels,
/// sample rate, bits a sample, and its samples' bytes.
fn wav(data: &[u8]) -> Option<(u16, u16, u32, u16, &[u8])> {
    if data.len() < 12 || &data[..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return None;
    }
    let (mut format, mut body) = (None, None);
    let mut at = 12;
    while at + 8 <= data.len() {
        let len = u32::from_le_bytes(data[at + 4..at + 8].try_into().ok()?) as usize;
        let chunk = &data[at + 8..(at + 8).saturating_add(len).min(data.len())];
        let half = |i: usize| u16::from_le_bytes([chunk[i], chunk[i + 1]]);
        match &data[at..at + 4] {
            b"fmt " if chunk.len() >= 16 => {
                let tag = if half(0) == 0xFFFE && chunk.len() >= 26 { half(24) } else { half(0) };
                format = Some((tag, half(2), u32::from_le_bytes(chunk[4..8].try_into().ok()?), half(14)));
            }
            b"data" => body = Some(chunk),
            _ => {}
        }
        // Chunks pad to an even length.
        at = at.saturating_add(8 + len + (len & 1));
    }
    let (tag, channels, rate, bits) = format?;
    Some((tag, channels, rate, bits, body?))
}

/// XFL's `format` ("22kHz 16bit Mono") as (rate, bits, channels).
fn raw_format(format: &str) -> Option<(u32, u16, u16)> {
    let mut words = format.split_whitespace();
    let khz: u32 = words.next()?.strip_suffix("kHz")?.parse().ok()?;
    let bits: u16 = words.next()?.strip_suffix("bit")?.parse().ok()?;
    let channels = if words.next()? == "Stereo" { 2 } else { 1 };
    // Animate's rates are the CD rate's fractions; "5kHz" is 5512.5 Hz.
    let rate = match khz {
        5 => 5512,
        11 => 11025,
        22 => 22050,
        44 => 44100,
        k => k * 1000,
    };
    matches!(bits, 8 | 16).then_some((rate, bits, channels))
}

/// Little-endian samples, `float` or signed (unsigned at 8 bits), as 16-bit ones. Only the
/// first two channels play.
// ponytail: past two channels the rest are dropped rather than mixed down.
fn samples(body: &[u8], float: bool, bits: u16, channels: u16, rate: u32) -> Pcm {
    let width = (bits as usize / 8).max(1);
    let n = channels.max(1) as usize;
    let kept = n.min(2);
    let mut samples = Vec::with_capacity(body.len() / width);
    for frame in body.chunks_exact(width * n) {
        for b in frame.chunks_exact(width).take(kept) {
            samples.push(match (float, width) {
                (false, 1) => (b[0] as i16 - 128) << 8,
                (false, _) => i16::from_le_bytes([b[width - 2], b[width - 1]]),
                (true, 4) => (f32::from_le_bytes([b[0], b[1], b[2], b[3]]).clamp(-1.0, 1.0) * 32767.0) as i16,
                (true, _) => (f64::from_le_bytes(b[..8].try_into().unwrap_or_default()).clamp(-1.0, 1.0) * 32767.0) as i16,
            });
        }
    }
    Pcm { rate, channels: kept as u16, samples }
}

/// An MP3's audio frames: its tags and any Xing/Info frame left out.
pub(crate) fn mp3_frames(data: &[u8]) -> impl Iterator<Item = nanomp3::Frame<'_>> {
    nanomp3::frames(data).filter(|f| nanomp3::VbrTag::parse(f.data).is_none())
}

// ---- What keyframes play ------------------------------------------------------------------

/// When a keyframe's sound plays (Properties > Frame > Sound > Sync).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sync {
    /// Every time the keyframe is reached, to the end, whatever the timeline does.
    Event,
    /// As Event, unless the sound is playing already.
    Start,
    /// Stops the sound.
    Stop,
    /// Tied to the timeline: it plays over the keyframe's frames only, in step with them.
    Stream,
}

/// Properties > Frame > Sound > Effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    LeftChannel,
    RightChannel,
    FadeLeftToRight,
    FadeRightToLeft,
    FadeIn,
    FadeOut,
    /// The keyframe's own envelope.
    Custom,
}

impl Effect {
    pub const ALL: [(Effect, &'static str); 8] = [
        (Effect::None, "none"),
        (Effect::LeftChannel, "left channel"),
        (Effect::RightChannel, "right channel"),
        (Effect::FadeLeftToRight, "fade left to right"),
        (Effect::FadeRightToLeft, "fade right to left"),
        (Effect::FadeIn, "fade in"),
        (Effect::FadeOut, "fade out"),
        (Effect::Custom, "custom"),
    ];
}

/// The sound a keyframe plays.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameSound {
    /// The library sound's name.
    pub name: String,
    pub sync: Sync,
    /// Loops for as long as it can, rather than playing `repeat` times.
    pub looping: bool,
    pub repeat: u32,
    pub effect: Effect,
    /// Where it starts and ends, in 44.1 kHz samples (XFL's unit and SWF's, whatever the rate).
    pub in_point: u32,
    pub out_point: Option<u32>,
    /// Volume points as XFL keeps them: (44.1 kHz sample, left, right), levels 0..=1. A
    /// Custom effect's own; for the others Animate may write out the points they make.
    pub envelope: Vec<(u32, f32, f32)>,
}

impl FrameSound {
    /// Plays `name` once, as an event, as a sound picked in Properties starts.
    pub fn new(name: &str) -> FrameSound {
        FrameSound { name: name.into(), sync: Sync::Event, looping: false, repeat: 1, effect: Effect::None, in_point: 0, out_point: None, envelope: Vec::new() }
    }

    /// The volume points it plays with, (44.1 kHz sample, left, right): its envelope, else
    /// what its effect makes of a sound `length` samples long, fades running from its in
    /// point to its out point; none plays at full volume.
    pub fn volumes(&self, length: u32) -> Vec<(u32, f32, f32)> {
        if !self.envelope.is_empty() {
            return self.envelope.clone();
        }
        let (a, b) = (self.in_point, self.out_point.unwrap_or(length).max(self.in_point + 1));
        match self.effect {
            Effect::None | Effect::Custom => Vec::new(),
            Effect::LeftChannel => vec![(0, 1.0, 0.0)],
            Effect::RightChannel => vec![(0, 0.0, 1.0)],
            Effect::FadeLeftToRight => vec![(a, 1.0, 0.0), (b, 0.0, 1.0)],
            Effect::FadeRightToLeft => vec![(a, 0.0, 1.0), (b, 1.0, 0.0)],
            Effect::FadeIn => vec![(a, 0.0, 0.0), (b, 1.0, 1.0)],
            Effect::FadeOut => vec![(a, 1.0, 1.0), (b, 0.0, 0.0)],
        }
    }
}

/// The left and right volume `points` (as `FrameSound::volumes` gives them) set at
/// `at` 44.1 kHz samples in: straight between points, level before the first and after the last.
pub fn volume(points: &[(u32, f32, f32)], at: f64) -> (f32, f32) {
    let Some(next) = points.iter().position(|p| p.0 as f64 > at) else {
        return points.last().map_or((1.0, 1.0), |p| (p.1, p.2));
    };
    if next == 0 {
        return (points[0].1, points[0].2);
    }
    let (a, b) = (points[next - 1], points[next]);
    let t = ((at - a.0 as f64) / (b.0 as f64 - a.0 as f64)) as f32;
    (a.1 + (b.1 - a.1) * t, a.2 + (b.2 - a.2) * t)
}

/// The DOMFrame attributes a sound takes; its envelope is a `<SoundEnvelope>` child.
// ponytail: names as Animate's XFL and JSFL use them; inPoint44, outPoint44 and the envelope's
// SoundEnvelopePoint mark44/level0/level1 are from saved files, unconfirmed by a spec.
const ATTRS: [&str; 7] = ["soundName", "soundSync", "soundLoopMode", "soundLoop", "soundEffect", "inPoint44", "outPoint44"];

fn is_envelope(xml: &str) -> bool {
    xml.trim_start().starts_with("<SoundEnvelope")
}

impl Frame {
    /// The sound this keyframe plays, if any.
    pub fn sound(&self) -> Option<FrameSound> {
        let e = &self.extra;
        let name = e.get("soundName").filter(|n| !n.is_empty())?;
        let num = |k: &str| e.get(k).and_then(|v| v.parse::<u32>().ok());
        let envelope = e.before.iter().chain(&e.after).find(|x| is_envelope(x)).map_or_else(Vec::new, |xml| read_envelope(xml));
        Some(FrameSound {
            name: name.into(),
            sync: match e.get("soundSync") {
                Some("start") => Sync::Start,
                Some("stop") => Sync::Stop,
                Some("stream") => Sync::Stream,
                _ => Sync::Event,
            },
            looping: e.get("soundLoopMode") == Some("loop"),
            repeat: num("soundLoop").unwrap_or(1).max(1),
            effect: e.get("soundEffect").and_then(|v| Effect::ALL.iter().find(|a| a.1 == v)).map_or(Effect::None, |a| a.0),
            in_point: num("inPoint44").unwrap_or(0),
            out_point: num("outPoint44"),
            envelope,
        })
    }

    /// Gives the keyframe sound `s`, or none.
    pub fn set_sound(&mut self, s: Option<&FrameSound>) {
        let e = &mut self.extra;
        e.attrs.retain(|(k, _)| !ATTRS.contains(&k.as_str()));
        e.before.retain(|x| !is_envelope(x));
        e.after.retain(|x| !is_envelope(x));
        let Some(s) = s else { return };
        let sync = match s.sync {
            Sync::Event => None,
            Sync::Start => Some("start"),
            Sync::Stop => Some("stop"),
            Sync::Stream => Some("stream"),
        };
        let effect = Effect::ALL.iter().find(|a| a.0 == s.effect).map(|a| a.1).filter(|&v| v != "none");
        let values = [
            ("soundName", Some(s.name.clone())),
            ("soundSync", sync.map(String::from)),
            ("soundLoopMode", s.looping.then(|| "loop".to_string())),
            ("soundLoop", (s.repeat != 1).then(|| s.repeat.to_string())),
            ("soundEffect", effect.map(String::from)),
            ("inPoint44", (s.in_point != 0).then(|| s.in_point.to_string())),
            ("outPoint44", s.out_point.map(|p| p.to_string())),
        ];
        for (k, v) in values {
            if let Some(v) = v {
                e.attrs.push((k.into(), v));
            }
        }
        if !s.envelope.is_empty() {
            let level = |v: f32| (v.clamp(0.0, 1.0) * 32768.0).round() as u32;
            let points: String = s.envelope.iter().map(|&(at, l, r)| format!("<SoundEnvelopePoint mark44=\"{at}\" level0=\"{}\" level1=\"{}\"/>", level(l), level(r))).collect();
            e.before.push(format!("<SoundEnvelope>{points}</SoundEnvelope>"));
        }
    }
}

fn read_envelope(xml: &str) -> Vec<(u32, f32, f32)> {
    let Ok(doc) = roxmltree::Document::parse(xml) else { return Vec::new() };
    let num = |n: roxmltree::Node, k: &str, default: u32| n.attribute(k).and_then(|v| v.parse().ok()).unwrap_or(default);
    let level = |n, k| num(n, k, 32768) as f32 / 32768.0;
    doc.descendants().filter(|n| n.has_tag_name("SoundEnvelopePoint")).map(|n| (num(n, "mark44", 0), level(n, "level0"), level(n, "level1"))).collect()
}

/// Every keyframe sound of `tls`, played one after another as a document's scenes are: the
/// frame it starts on, counted from the first timeline's start, its keyframe and the sound.
/// Guide layers never publish, so they play nothing.
pub fn sounds(tls: &[Timeline]) -> Vec<(u32, &Frame, FrameSound)> {
    let mut out = Vec::new();
    let mut start = 0;
    for tl in tls {
        for layer in tl.layers.iter().filter(|l| l.kind != LayerKind::Guide) {
            out.extend(layer.frames.iter().filter_map(|k| Some((start + k.index, k, k.sound()?))));
        }
        start += tl.frame_count();
    }
    out
}

/// Plays `s`, a keyframe's settings for `pcm`, into `out` (interleaved stereo, -1..=1),
/// adding to what's there: from its in point to its out point, as many times as it repeats,
/// at the volumes its effect gives, until `out` is full.
pub fn mix(pcm: &Pcm, s: &FrameSound, out: &mut [f32]) {
    let rate = pcm.rate.max(1) as u64;
    let to_samples = |at44: u32| ((at44 as u64 * rate / 44100) as usize).min(pcm.len());
    let (from, to) = (to_samples(s.in_point), s.out_point.map_or(pcm.len(), to_samples));
    if from >= to {
        return;
    }
    let points = s.volumes((pcm.len() as u64 * 44100 / rate) as u32);
    let mut k = 0;
    for _ in 0..if s.looping { u32::MAX } else { s.repeat.max(1) } {
        for i in from..to {
            if k + 1 >= out.len() {
                return;
            }
            let (l, r) = volume(&points, i as f64 * 44100.0 / rate as f64);
            out[k] += pcm.at(i, 0) as f32 / 32768.0 * l;
            out[k + 1] += pcm.at(i, 1) as f32 / 32768.0 * r;
            k += 2;
        }
    }
}

// ---- XFL ------------------------------------------------------------------------------------

/// Where the bytes of the sound `name`, whose item has `extra`, may be in the XFL: the file
/// it was imported from, in LIBRARY/ (`href`, else its name), and Animate's copy in bin/
/// (`soundDataHRef`), any name that would lead out of the XFL escaped.
fn places(name: &str, extra: &Extra) -> [Option<String>; 2] {
    let library = safe(extra.get("href").unwrap_or(name)).map(|h| format!("LIBRARY/{h}"));
    [library, extra.get("soundDataHRef").and_then(safe).map(|h| format!("bin/{h}"))]
}

/// Where sound `name`'s bytes go: a WAV or MP3 file to LIBRARY/, bare samples back to bin/.
fn file(name: &str, s: &Sound) -> Option<String> {
    let [library, bin] = places(name, &s.extra);
    if is_file(&s.data) { library } else { bin.or(library) }
}

/// Whether `data` is a WAV or MP3 file, not bare samples.
fn is_file(data: &[u8]) -> bool {
    wav(data).is_some() || nanomp3::detect(data)
}

/// `href` as a path that stays in the XFL: `xfl_write::href`'s escapes but `%`'s own, so a
/// path made safe stays as it is when it's read back.
fn safe(href: &str) -> Option<String> {
    crate::xfl_write::href(href).ok().map(|h| h.replace("%25", "%"))
}

/// A `<DOMSoundItem>`, given as its XML, as a library sound and its name, its bytes
/// read by path in the XFL with `load`; None if they can't be found.
pub(crate) fn read_item(xml: &str, load: &mut dyn FnMut(&str) -> Result<Vec<u8>>) -> Option<(String, Sound)> {
    let parsed = roxmltree::Document::parse(xml).ok()?;
    let item = parsed.root_element();
    let name = item.attribute("name")?;
    let mut extra = Extra::default();
    for a in item.attributes().filter(|a| a.name() != "name") {
        extra.attrs.push((xml[a.range_qname()].into(), a.value().into()));
    }
    // The imported file, unless it's gone or isn't one: the copy in bin/ has a layout that isn't known.
    let [library, bin] = places(name, &extra);
    let imported = library.and_then(|f| load(&f).ok()).filter(|d| bin.is_none() || is_file(d));
    let data = imported.or_else(|| load(&bin?).ok())?;
    Some((name.into(), Sound { data: data.into(), extra }))
}

/// Sound `name`'s `<DOMSoundItem>`, naming the file `files` writes its bytes to.
pub(crate) fn item_xml(name: &str, s: &Sound) -> String {
    let mut attrs = vec![("name".to_string(), name.to_string())];
    attrs.extend(s.extra.attrs.iter().cloned());
    let file = file(name, s).unwrap_or_default();
    let (dir, path) = file.split_once('/').unwrap_or_default();
    let key = if dir == "bin" { "soundDataHRef" } else { "href" };
    match attrs.iter_mut().find(|(k, _)| k == key) {
        Some(slot) => slot.1 = path.into(),
        None => attrs.push((key.into(), path.into())),
    }
    let attrs: String = attrs.iter().map(|(k, v)| format!(" {k}=\"{}\"", crate::xfl_write::escape(v))).collect();
    format!("<DOMSoundItem{attrs}/>")
}

/// Takes the `<DOMSoundItem>`s out of the media list (bitmap.rs keeps them there unread)
/// into `doc.sounds`. One whose bytes can't be found stays in the list, unread.
pub(crate) fn read(doc: &mut Document, load: &mut dyn FnMut(&str) -> Result<Vec<u8>>) {
    let sounds = &mut doc.sounds;
    doc.media.retain(|m| match m {
        Media::Unsupported { name, xml } if name == "DOMSoundItem" => match read_item(xml, load) {
            Some((name, sound)) => {
                sounds.insert(name, sound);
                false
            }
            None => true,
        },
        _ => true,
    });
}

/// The library's sounds as `<DOMSoundItem>`s, for the media list (bitmap::write_media).
pub(crate) fn items(doc: &Document) -> Vec<String> {
    doc.sounds.iter().map(|(name, s)| item_xml(name, s)).collect()
}

/// The sounds' files, by path in the XFL.
pub(crate) fn files(doc: &Document) -> Vec<(String, Vec<u8>)> {
    doc.sounds.iter().filter_map(|(name, s)| Some((file(name, s)?, s.data.to_vec()))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 16-bit WAV file of `samples` (interleaved when `channels` is 2).
    pub(crate) fn wav_file(rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
        let body: Vec<u8> = samples.iter().flat_map(|s| s.to_le_bytes()).collect();
        let mut out = Vec::new();
        out.extend(b"RIFF");
        out.extend((36 + body.len() as u32).to_le_bytes());
        out.extend(b"WAVEfmt ");
        out.extend(16u32.to_le_bytes());
        out.extend(1u16.to_le_bytes());
        out.extend(channels.to_le_bytes());
        out.extend(rate.to_le_bytes());
        out.extend((rate * channels as u32 * 2).to_le_bytes());
        out.extend((channels * 2).to_le_bytes());
        out.extend(16u16.to_le_bytes());
        out.extend(b"data");
        out.extend((body.len() as u32).to_le_bytes());
        out.extend(body);
        out
    }

    #[test]
    fn wav_files_decode() {
        let s = Sound::from_file(wav_file(22050, 2, &[1, -1, 300, -300, i16::MAX, i16::MIN]), "a.wav").unwrap();
        let info = s.info().unwrap();
        assert_eq!((info.codec, info.rate, info.channels, info.bits, info.samples), (Codec::Wav, 22050, 2, 16, 3));
        assert_eq!(s.extra.get("format"), Some("22kHz 16bit Stereo"));
        let pcm = s.decode().unwrap();
        assert_eq!((pcm.at(1, 0), pcm.at(1, 1), pcm.len()), (300, -300, 3));
        // Bare samples, as `format` lays them out.
        let raw = Sound { data: Arc::from(&[0u8, 0x80, 0xFF, 0x7F][..]), extra: Extra { attrs: vec![("format".into(), "5kHz 16bit Mono".into())], ..Extra::default() } };
        assert_eq!(raw.info().map(|i| (i.codec, i.rate, i.samples)), Some((Codec::Raw, 5512, 2)));
        assert_eq!(raw.decode().unwrap().samples, [i16::MIN, i16::MAX]);
        assert!(Sound::from_file(b"RIFF....WAVEjunk".to_vec(), "b.wav").is_err());
        // Resampled, the length follows the rate.
        let pcm = Pcm { rate: 10, channels: 1, samples: vec![0, 100, 200, 300] };
        assert_eq!(pcm.resampled(20).samples, [0, 50, 100, 150, 200, 250, 300, 300]);
    }

    #[test]
    fn frame_sounds_live_in_the_frames_attributes() {
        let mut f = Timeline::new("s").layers.remove(0).frames.remove(0);
        assert_eq!(f.sound(), None);
        let s = FrameSound {
            sync: Sync::Stream,
            looping: true,
            repeat: 3,
            effect: Effect::Custom,
            in_point: 441,
            out_point: Some(44100),
            envelope: vec![(0, 0.0, 1.0), (22050, 1.0, 0.5)],
            ..FrameSound::new("Sounds/beep.wav")
        };
        f.set_sound(Some(&s));
        assert_eq!(f.sound().as_ref(), Some(&s));
        assert_eq!(f.extra.get("soundSync"), Some("stream"));
        assert_eq!(f.extra.before.len(), 1);
        f.set_sound(Some(&FrameSound::new("beep.wav")));
        assert_eq!(f.extra.attrs, [("soundName".to_string(), "beep.wav".to_string())], "defaults aren't written");
        assert!(f.extra.before.is_empty());
        f.set_sound(None);
        assert_eq!(f.extra, Extra::default());
    }

    #[test]
    fn effects_make_volume_points() {
        let fade = FrameSound { effect: Effect::FadeIn, ..FrameSound::new("a") };
        let points = fade.volumes(1000);
        assert_eq!(volume(&points, 0.0), (0.0, 0.0));
        assert_eq!(volume(&points, 500.0), (0.5, 0.5));
        assert_eq!(volume(&points, 2000.0), (1.0, 1.0));
        assert_eq!(volume(&[], 7.0), (1.0, 1.0));
        let left = FrameSound { effect: Effect::LeftChannel, ..FrameSound::new("a") };
        let mut out = vec![0.0; 8];
        mix(&Pcm { rate: 44100, channels: 1, samples: vec![16384; 3] }, &left, &mut out);
        assert_eq!(out, [0.5, 0.0, 0.5, 0.0, 0.5, 0.0, 0.0, 0.0], "a mono sound in the left channel, played once");
    }
}
