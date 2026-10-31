//! Sounds in a SWF: each sound the timelines cue is defined once, as uncompressed 16-bit PCM,
//! and every cue becomes a `StartSound` before the `ShowFrame` of its frame.
//!
//! Uncompressed keeps this free of an encoder and exact to what the editor plays; the price is
//! size, which the diagnostics say. A SWF holds only four sample rates, so a sound is resampled
//! to the nearest one at or above its own (never below, which would lose its highs).

use std::collections::HashMap;

use flare_core::SoundId;
use flare_document::Document;
use swf::{AudioCompression, CharacterId, SoundEvent, SoundFormat, SoundInfo, StartSound, Tag};

use crate::{Diagnostic, Severity};

/// The most samples all the defined sounds may hold together (about 12 minutes of stereo at
/// 44.1 kHz, 128 MiB), so a document cannot make the publisher grow without bound.
pub(crate) const MAX_SAMPLES: usize = 64 * 1024 * 1024;

/// The sample rates a SWF can hold.
const RATES: [u32; 4] = [5512, 11025, 22050, 44100];

/// One sound as it goes into the file. A `DefineSound` tag borrows its bytes, so these are kept
/// here and the tags made from them when the file is written.
pub(crate) struct Defined {
    id: CharacterId,
    format: SoundFormat,
    num_samples: u32,
    data: Vec<u8>,
}

impl Defined {
    pub fn tag(&self) -> Tag<'_> {
        Tag::DefineSound(swf::Sound {
            id: self.id,
            format: self.format.clone(),
            num_samples: self.num_samples,
            data: &self.data,
        })
    }
}

/// What publishing found to write about sounds.
#[derive(Default)]
pub(crate) struct Sounds {
    /// To go before the first frame.
    pub define: Vec<Defined>,
    /// `StartSound` tags by the frame (counted across all scenes) they go before.
    pub starts: HashMap<u32, Vec<StartSound>>,
    pub diagnostics: Vec<Diagnostic>,
    /// The first character id nothing else has taken.
    pub next_character: CharacterId,
}

fn rate_for(source: u32) -> u32 {
    RATES
        .iter()
        .copied()
        .find(|rate| *rate >= source)
        .unwrap_or(RATES[3])
}

fn warn(message: String) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        message,
    }
}

/// Defines the sounds the document cues and places their starts. `first` is the first free
/// character id; `limit` is [`MAX_SAMPLES`] outside tests.
pub(crate) fn define(document: &Document, first: CharacterId, limit: usize) -> Sounds {
    let mut out = Sounds {
        next_character: first,
        ..Sounds::default()
    };
    let mut cues: Vec<(u32, SoundId)> = Vec::new();
    let mut offset = 0_u32;
    for scene in document.scenes() {
        let length = u32::try_from(scene.timeline().frame_count().max(1)).unwrap_or(u32::MAX);
        for layer in scene.timeline().layers() {
            for cue in layer.cues().iter().filter(|cue| cue.start < length) {
                cues.push((offset.saturating_add(cue.start), cue.sound));
            }
        }
        offset = offset.saturating_add(length);
    }
    if cues.is_empty() {
        return out;
    }
    cues.sort_by_key(|(frame, _)| *frame);

    let mut defined: HashMap<SoundId, Option<CharacterId>> = HashMap::new();
    let mut samples_left = limit;
    let mut wrote = false;
    for (frame, id) in cues {
        let character = match defined.get(&id) {
            Some(known) => *known,
            None => {
                let made = match document.sound(id) {
                    None => {
                        out.diagnostics
                            .push(warn("a sound cue names a sound the document lacks".into()));
                        None
                    }
                    Some(sound) => build(sound, out.next_character, &mut samples_left)
                        .map_err(|why| {
                            out.diagnostics.push(warn(format!(
                                "sound {:?} was not published: {why}",
                                sound.name()
                            )));
                        })
                        .ok(),
                };
                let assigned = made.map(|tag| {
                    let character = out.next_character;
                    out.next_character = out.next_character.saturating_add(1);
                    out.define.push(tag);
                    character
                });
                defined.insert(id, assigned);
                assigned
            }
        };
        if let Some(sound) = character {
            wrote = true;
            out.starts.entry(frame).or_default().push(StartSound {
                id: sound,
                sound_info: Box::new(SoundInfo {
                    event: SoundEvent::Event,
                    in_sample: None,
                    out_sample: None,
                    num_loops: 1,
                    envelope: None,
                }),
            });
        }
    }
    if wrote {
        out.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            message: "sounds were written as uncompressed 16-bit audio at the nearest SWF sample rate, so the file is larger than the originals".into(),
        });
    }
    out
}

fn build(
    sound: &flare_document::Sound,
    id: CharacterId,
    samples_left: &mut usize,
) -> Result<Defined, String> {
    if id == CharacterId::MAX {
        return Err("a SWF holds at most 65535 characters".into());
    }
    let audio = flare_audio::decode(sound.data()).map_err(|e| e.to_string())?;
    let rate = rate_for(audio.sample_rate());
    let (pcm, channels) = audio
        .to_pcm16(rate, *samples_left)
        .ok_or_else(|| "it is too long for one SWF".to_string())?;
    *samples_left -= pcm.len();
    let num_samples = u32::try_from(pcm.len() / usize::from(channels))
        .map_err(|_| "it is too long for one SWF".to_string())?;
    let data: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    Ok(Defined {
        id,
        format: SoundFormat {
            compression: AudioCompression::Uncompressed,
            sample_rate: rate as u16,
            is_stereo: channels == 2,
            is_16_bit: true,
        },
        num_samples,
        data,
    })
}

#[cfg(test)]
#[path = "sound_tests.rs"]
mod tests;
