//! Sounds in the SWF (swf_export.rs): a DefineSound for each sound keyframes start or
//! stop, a StartSound on the frames that do, and each timeline's stream sounds mixed
//! into one stream: a SoundStreamHead2, then a SoundStreamBlock a frame.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use swf::{AudioCompression, CharacterId, SoundEnvelopePoint, SoundEvent, SoundFormat, SoundInfo, SoundStreamHead, StartSound, Tag};

use crate::Result;
use crate::doc::{Document, Frame, Timeline};
use crate::sound::{self, Compression, FrameSound, Sound, Sync};

/// The document's sounds as the SWF holds them.
pub(crate) struct Sounds {
    /// What each DefineSound holds, by sound name.
    defs: BTreeMap<String, Def>,
    /// Each timeline's stream, by the address of the timelines it plays (`add`'s `tls`).
    streams: HashMap<usize, Stream>,
}

struct Def {
    id: CharacterId,
    format: SoundFormat,
    samples: u32,
    data: Vec<u8>,
}

struct Stream {
    head: SoundStreamHead,
    /// Each frame's block, by frame.
    blocks: BTreeMap<u32, Vec<u8>>,
}

impl Sounds {
    /// Codes the sounds the document's timelines play: the main one (its scenes one after
    /// another) and each symbol's. What can't be coded is added to `skipped`.
    pub(crate) fn new(doc: &Document, skipped: &mut BTreeSet<String>) -> Sounds {
        let symbols = doc.library.values().map(|s| std::slice::from_ref(&s.timeline));
        let (mut used, mut streams) = (BTreeSet::new(), HashMap::new());
        for tls in std::iter::once(&doc.scenes[..]).chain(symbols) {
            let sounds = sound::sounds(tls);
            used.extend(sounds.iter().filter(|(.., s)| s.sync != Sync::Stream).map(|(.., s)| s.name.clone()));
            if let Some(stream) = stream(doc, &sounds, skipped) {
                streams.insert(tls.as_ptr() as usize, stream);
            }
        }
        // Ids count down from the top, clear of those swf_export counts up from 1.
        // ponytail: the two would meet only past 65,535 characters in all.
        let mut defs = BTreeMap::new();
        for (i, name) in used.into_iter().enumerate() {
            let coded = match doc.sounds.get(&name) {
                Some(s) => define(s).map_err(|e| format!("sound {name:?} ({e})")),
                None => Err(format!("missing sound {name:?}")),
            };
            match coded {
                Ok((format, samples, data)) => {
                    defs.insert(name, Def { id: u16::MAX - i as u16, format, samples, data });
                }
                Err(e) => {
                    skipped.insert(e);
                }
            }
        }
        Sounds { defs, streams }
    }

    /// The DefineSound tags, to go before anything that plays them.
    pub(crate) fn defines(&self) -> Vec<Tag<'_>> {
        let defs = self.defs.values();
        defs.map(|d| Tag::DefineSound(swf::Sound { id: d.id, format: d.format.clone(), num_samples: d.samples, data: &d.data })).collect()
    }

    /// `tags`, the control tags playing `tls` (a ShowFrame ending each frame), with the
    /// sounds their keyframes start, stop and stream put in.
    pub(crate) fn add<'a>(&'a self, tls: &[Timeline], tags: Vec<Tag<'a>>) -> Vec<Tag<'a>> {
        let sounds = sound::sounds(tls);
        if sounds.is_empty() {
            return tags;
        }
        let stream = self.streams.get(&(tls.as_ptr() as usize));
        let mut out = Vec::with_capacity(tags.len() + sounds.len());
        out.extend(stream.map(|s| Tag::SoundStreamHead2(s.head.clone())));
        let mut frame = 0;
        for tag in tags {
            if matches!(tag, Tag::ShowFrame) {
                for (_, _, s) in sounds.iter().filter(|(f, ..)| *f == frame) {
                    if let Some(def) = self.defs.get(&s.name).filter(|_| s.sync != Sync::Stream) {
                        out.push(Tag::StartSound(StartSound { id: def.id, sound_info: Box::new(info(s, def)) }));
                    }
                }
                out.extend(stream.and_then(|s| s.blocks.get(&frame)).map(|b| Tag::SoundStreamBlock(b)));
                frame += 1;
            }
            out.push(tag);
        }
        out
    }
}

/// How StartSound plays `s`.
fn info(s: &FrameSound, def: &Def) -> SoundInfo {
    let length = (def.samples as u64 * 44100 / def.format.sample_rate.max(1) as u64) as u32;
    let points = s.volumes(length);
    let envelope = points.iter().take(255).map(|&(sample, l, r)| SoundEnvelopePoint { sample, left_volume: l, right_volume: r });
    SoundInfo {
        event: match s.sync {
            Sync::Start => SoundEvent::Start,
            Sync::Stop => SoundEvent::Stop,
            Sync::Event | Sync::Stream => SoundEvent::Event,
        },
        in_sample: (s.in_point != 0).then_some(s.in_point),
        out_sample: s.out_point,
        // Looping goes on as long as SWF counts.
        num_loops: if s.looping { u16::MAX } else { s.repeat.clamp(1, u16::MAX as u32) as u16 },
        envelope: (!points.is_empty()).then(|| envelope.collect()),
    }
}

/// What a DefineSound holds of `s`: an MP3 at a rate SWF has, as it is; else ADPCM (plain
/// 16-bit samples with Raw compression) at the SWF rate nearest above its own.
fn define(s: &Sound) -> Result<(SoundFormat, u32, Vec<u8>)> {
    let c = s.compression();
    if matches!(c, Compression::Default | Compression::Mp3)
        && let Some(mp3) = mp3(&s.data)
    {
        return Ok(mp3);
    }
    let pcm = s.decode()?;
    let pcm = pcm.resampled(swf_rate(pcm.rate))?;
    let format = |compression| SoundFormat { compression, sample_rate: pcm.rate as u16, is_stereo: pcm.channels == 2, is_16_bit: true };
    Ok(match c {
        Compression::Raw => (format(AudioCompression::Uncompressed), pcm.len() as u32, pcm.samples.iter().flat_map(|v| v.to_le_bytes()).collect()),
        Compression::Adpcm(bits) => (format(AudioCompression::Adpcm), pcm.len() as u32, adpcm(&pcm.samples, pcm.channels, bits, &mut [0; 2])),
        Compression::Default | Compression::Mp3 => (format(AudioCompression::Adpcm), pcm.len() as u32, adpcm(&pcm.samples, pcm.channels, 4, &mut [0; 2])),
    })
}

/// The SWF rate (5.5, 11, 22 or 44 kHz) nearest at or above `rate`.
fn swf_rate(rate: u32) -> u32 {
    [5512, 11025, 22050, 44100].into_iter().find(|&r| r >= rate).unwrap_or(44100)
}

/// An MP3 as DefineSound holds it, if all its frames are at a rate SWF has: no seek, then the frames.
fn mp3(data: &[u8]) -> Option<(SoundFormat, u32, Vec<u8>)> {
    let frames: Vec<_> = sound::mp3_frames(data).collect();
    let first = frames.first()?;
    let alike = |f: &nanomp3::Frame| f.layer == 3 && f.sample_rate == first.sample_rate && f.channels.num() == first.channels.num();
    if !frames.iter().all(alike) || !matches!(first.sample_rate, 11025 | 22050 | 44100) {
        return None;
    }
    let mut out = vec![0, 0];
    frames.iter().for_each(|f| out.extend_from_slice(f.data));
    let format = SoundFormat { compression: AudioCompression::Mp3, sample_rate: first.sample_rate as u16, is_stereo: first.channels.num() == 2, is_16_bit: true };
    Some((format, frames.iter().map(|f| f.samples as u32).sum(), out))
}

/// The stream sounds among `sounds` (a timeline's, as `sound::sounds` lists them) mixed
/// into one stereo stream, each over its keyframe's frames, coded as ADPCM a frame a block.
// ponytail: an MP3 streams as ADPCM too; MP3 blocks (frames cut at frame boundaries, with a
// seek) would keep its own coding and size. The whole mix is held in memory, as floats.
fn stream(doc: &Document, sounds: &[(u32, &Frame, FrameSound)], skipped: &mut BTreeSet<String>) -> Option<Stream> {
    let fps = doc.frame_rate.max(0.01) as f64;
    let mut parts = Vec::new();
    for (start, key, s) in sounds.iter().filter(|(.., s)| s.sync == Sync::Stream) {
        let decoded = match doc.sounds.get(&s.name) {
            Some(sound) => sound.decode().map_err(|e| format!("sound {:?} ({e})", s.name)),
            None => Err(format!("missing sound {:?}", s.name)),
        };
        match decoded {
            Ok(pcm) => parts.push((*start, start + key.duration, pcm, s)),
            Err(e) => {
                skipped.insert(e);
            }
        }
    }
    let rate = parts.iter().map(|p| swf_rate(p.2.rate)).max()?;
    let at = |f: u32| (f as f64 * rate as f64 / fps).round() as usize;
    let (first, end) = (parts.iter().map(|p| p.0).min()?, parts.iter().map(|p| p.1).max()?);
    let mut mix = vec![0.0; at(end) * 2];
    for (start, stop, pcm, s) in &parts {
        sound::mix(&pcm.resampled(rate).ok()?, s, &mut mix[at(*start) * 2..at(*stop) * 2]);
    }
    // Each block opens at the step sizes the one before reached, or its first samples would
    // lag the sound, a click a frame.
    let mut index = [0; 2];
    let mut block = |f: u32| -> Vec<u8> {
        let samples: Vec<i16> = mix[at(f) * 2..at(f + 1) * 2].iter().map(|&v| (v.clamp(-1.0, 1.0) * 32767.0) as i16).collect();
        adpcm(&samples, 2, 4, &mut index)
    };
    let format = SoundFormat { compression: AudioCompression::Adpcm, sample_rate: rate as u16, is_stereo: true, is_16_bit: true };
    let per_frame = (rate as f64 / fps).round() as u16;
    let head = SoundStreamHead { stream_format: format.clone(), playback_format: format, num_samples_per_block: per_frame, latency_seek: 0 };
    Some(Stream { head, blocks: (first..end).map(|f| (f, block(f))).collect() })
}

// ---- ADPCM ------------------------------------------------------------------------------------

/// IMA ADPCM's step sizes, which SWF's ADPCM uses.
const STEPS: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66, 73, 80, 88, 97, 107, 118, 130, 143,
    157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408, 449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552,
    1707, 1878, 2066, 2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630, 9493, 10442, 11487,
    12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794, 32767,
];

/// How each code's magnitude moves the step index, by code size (SWF File Format Specification, ADPCM).
const INDEX: [&[i32]; 4] = [
    &[-1, 2],
    &[-1, -1, 2, 4],
    &[-1, -1, -1, -1, 2, 4, 6, 8],
    &[-1, -1, -1, -1, -1, -1, -1, -1, 1, 2, 4, 6, 8, 10, 13, 16],
];

/// Interleaved `samples` as SWF's ADPCM: the code size, then packets of 4096 samples a
/// channel, each opening with its first samples whole and the step indexes, the rest as
/// `bits`-bit codes (2 to 5), interleaved. Each channel's step index starts at its `index`
/// and is left there as it ends.
fn adpcm(samples: &[i16], channels: u16, bits: u8, index: &mut [i32]) -> Vec<u8> {
    let (n, bits) = (channels.max(1) as usize, bits.clamp(2, 5) as u32);
    let mut w = Bits::default();
    w.put(bits - 2, 2);
    // Each channel's predicted sample and step index.
    let mut state: Vec<(i32, i32)> = (0..n).map(|c| (0, index.get(c).copied().unwrap_or(0))).collect();
    for packet in samples.chunks(4096 * n) {
        for (c, st) in state.iter_mut().enumerate() {
            let first = packet.get(c).copied().unwrap_or(0);
            // The header has 6 bits for the index.
            *st = (first as i32, st.1.min(63));
            w.put(first as u16 as u32, 16);
            w.put(st.1 as u32, 6);
        }
        for frame in packet.chunks(n).skip(1) {
            for (c, &v) in frame.iter().enumerate() {
                let code = code(&mut state[c], v as i32, bits);
                w.put(code, bits);
            }
        }
    }
    index.iter_mut().zip(&state).for_each(|(i, st)| *i = st.1);
    w.finish()
}

/// The code for `sample`, moving `st` (predicted sample, step index) on as a player decoding
/// it does: its delta is IMA's sum of halved steps.
fn code(st: &mut (i32, i32), sample: i32, bits: u32) -> u32 {
    let step = STEPS[st.1 as usize];
    let diff = sample - st.0;
    let (mut left, mut magnitude, mut delta) = (diff.abs(), 0, step >> (bits - 1));
    for i in 1..bits {
        let part = step >> (i - 1);
        if left >= part {
            magnitude |= 1 << (bits - 1 - i);
            left -= part;
            delta += part;
        }
    }
    st.0 = (if diff < 0 { st.0 - delta } else { st.0 + delta }).clamp(-32768, 32767);
    st.1 = (st.1 + INDEX[bits as usize - 2][magnitude as usize]).clamp(0, 88);
    magnitude | if diff < 0 { 1 << (bits - 1) } else { 0 }
}

/// Bits written most significant first.
#[derive(Default)]
struct Bits {
    out: Vec<u8>,
    acc: u8,
    n: u32,
}

impl Bits {
    fn put(&mut self, value: u32, bits: u32) {
        for i in (0..bits).rev() {
            self.acc = self.acc << 1 | (value >> i & 1) as u8;
            self.n += 1;
            if self.n == 8 {
                self.out.push(self.acc);
                (self.acc, self.n) = (0, 0);
            }
        }
    }

    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc << (8 - self.n));
        }
        self.out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SWF's ADPCM decoded, as a player does.
    fn decode(data: &[u8], channels: usize) -> Vec<i16> {
        let mut at = 0;
        let mut read = |bits: u32| -> Option<u32> {
            let mut v = 0;
            for _ in 0..bits {
                let byte = *data.get(at / 8)?;
                v = v << 1 | (byte >> (7 - at % 8) & 1) as u32;
                at += 1;
            }
            Some(v)
        };
        let bits = read(2).unwrap() + 2;
        let mut out = Vec::new();
        'packets: loop {
            let mut st = Vec::new();
            for _ in 0..channels {
                let (Some(sample), Some(index)) = (read(16), read(6)) else { break 'packets };
                st.push((sample as u16 as i16 as i32, index as i32));
                out.push(sample as u16 as i16);
            }
            for _ in 1..4096 {
                for s in st.iter_mut() {
                    let Some(code) = read(bits) else { break 'packets };
                    let (step, sign) = (STEPS[s.1 as usize], 1 << (bits - 1));
                    let magnitude = code & !sign;
                    let mut delta = step >> (bits - 1);
                    for i in 1..bits {
                        if magnitude & (1 << (bits - 1 - i)) != 0 {
                            delta += step >> (i - 1);
                        }
                    }
                    s.0 = (if code & sign != 0 { s.0 - delta } else { s.0 + delta }).clamp(-32768, 32767);
                    s.1 = (s.1 + INDEX[bits as usize - 2][magnitude as usize]).clamp(0, 88);
                    out.push(s.0 as i16);
                }
            }
        }
        out
    }

    #[test]
    fn adpcm_follows_the_sound() {
        let wave: Vec<i16> = (0..10_000).flat_map(|i| {
            let v = ((i as f32 * 0.05).sin() * 12_000.0) as i16;
            [v, -v / 2]
        }).collect();
        for bits in 2..=5u8 {
            let decoded = decode(&adpcm(&wave, 2, bits, &mut [0; 2]), 2);
            assert!(decoded.len() >= wave.len(), "{bits} bits: {} of {} samples", decoded.len(), wave.len());
            let err = wave.iter().zip(&decoded).map(|(a, b)| (*a as f64 - *b as f64).abs()).sum::<f64>() / wave.len() as f64;
            let limit = [300.0, 150.0, 80.0, 40.0][bits as usize - 2];
            assert!(err < limit, "{bits} bits: mean error {err}");
        }
        // Packets restart the prediction every 4096 samples a channel.
        assert_eq!(decode(&adpcm(&[5, 7, -9], 1, 4, &mut [0]), 1)[0], 5);
    }

    #[test]
    fn stream_blocks_start_at_the_step_size_reached() {
        // A loud 1 kHz tone, as bare samples, streamed over 12 frames.
        let tone: Vec<i16> = (0..22050).map(|i| ((i as f32 * 1000.0 * std::f32::consts::TAU / 22050.0).sin() * 12_000.0) as i16).collect();
        let format = crate::doc::Extra { attrs: vec![("format".into(), "22kHz 16bit Mono".into())], ..Default::default() };
        let mut doc = Document::new(550.0, 400.0, 24.0);
        doc.sounds.insert("tone".into(), Sound { data: tone.iter().flat_map(|s| s.to_le_bytes()).collect::<Vec<u8>>().into(), extra: format });
        let key = &mut doc.scenes[0].layers[0].frames[0];
        key.duration = 12;
        key.set_sound(Some(&FrameSound { sync: Sync::Stream, ..FrameSound::new("tone") }));
        let sounds = Sounds::new(&doc, &mut BTreeSet::new());
        let blocks = &sounds.streams[&(doc.scenes.as_ptr() as usize)].blocks;
        assert_eq!(blocks.len(), 12);
        // Past the first, a block's opening samples follow the tone as closely as the rest do.
        let mut at = 0;
        for (f, block) in blocks {
            let decoded = decode(block, 2);
            let worst = decoded.chunks(2).take(32).enumerate().map(|(i, s)| (s[0] as i32 - tone[at + i] as i32).abs()).max().unwrap();
            assert!(*f == 0 || worst < 1500, "frame {f} opens {worst} off");
            at += decoded.len() / 2;
        }
    }
}
