//! The sound of a movie: every sound cued in the first scene, mixed to stereo and encoded as
//! AAC-LC a frame at a time, so a long film is never held whole.

use std::collections::HashMap;
use std::sync::Arc;

use flare_core::SoundId;
use flare_document::Document;
use oxideav_aac::encoder::{EncoderConfig, StreamEncoder};

/// The rate the sound is mixed and encoded at.
pub(crate) const RATE: u32 = 48_000;
/// Samples per channel in one AAC frame.
pub(crate) const FRAME_SAMPLES: usize = 1024;
/// How many samples the AAC encoder holds back: what goes in at sample `n` comes out of a
/// decoder at sample `n + ENCODER_DELAY`. A test pins this against the encoder itself.
pub(crate) const ENCODER_DELAY: usize = 1024;
/// The same in seconds. The container here has no edit list to tell a player to skip that much,
/// so every picture after the first comes this late instead (the first is held that much
/// longer) and the two stay together. The container will not take sound that starts before the
/// first picture, which is why the first picture still starts at zero.
pub(crate) const DELAY_SECONDS: f64 = ENCODER_DELAY as f64 / RATE as f64;
/// The bit rate of the encoded sound.
const BITRATE: u32 = 160_000;

/// Hands out the movie's sound as ADTS frames, each with the time it starts at in seconds.
pub(crate) struct Track {
    mixer: flare_audio::Mixer,
    encoder: StreamEncoder,
    /// AAC frames handed out so far.
    index: usize,
    /// How many frames cover the movie.
    frames: usize,
}

impl Track {
    /// The track for the first scene of `document` over `seconds`, or `None` when it cues no
    /// sound at all (such a movie has no audio track).
    pub(crate) fn new(
        document: &Document,
        rate: (u32, u32),
        seconds: f64,
    ) -> Result<Option<Self>, String> {
        let Some(scene) = document.scenes().first() else {
            return Ok(None);
        };
        let mut mixer = flare_audio::Mixer::new(RATE);
        let mut decoded: HashMap<SoundId, Arc<flare_audio::Audio>> = HashMap::new();
        let seconds_per_frame = f64::from(rate.1) / f64::from(rate.0);
        let mut any = false;
        for (_, cue) in scene.timeline().cues() {
            let Some(sound) = document.sound(cue.sound) else {
                continue;
            };
            let audio = match decoded.get(&cue.sound) {
                Some(audio) => audio.clone(),
                None => {
                    let audio = flare_audio::decode(sound.data())
                        .map_err(|e| format!("Cannot read the sound \"{}\": {e}", sound.name()))?;
                    let audio = Arc::new(audio);
                    decoded.insert(cue.sound, audio.clone());
                    audio
                }
            };
            let delay = (f64::from(cue.start) * seconds_per_frame * f64::from(RATE)) as u64;
            mixer.play(audio, delay);
            any = true;
        }
        if !any {
            return Ok(None);
        }
        let encoder = StreamEncoder::new(EncoderConfig {
            sample_rate: RATE,
            channels: 2,
            bitrate: BITRATE,
        })
        .map_err(|e| format!("Cannot start the sound encoder: {e}"))?;
        let frames = (seconds * f64::from(RATE) / FRAME_SAMPLES as f64).ceil() as usize;
        Ok(Some(Self {
            mixer,
            encoder,
            index: 0,
            frames: frames.max(1),
        }))
    }

    /// When the next frame starts, in seconds, or `None` once the movie is covered.
    pub(crate) fn next_start(&self) -> Option<f64> {
        (self.index < self.frames).then(|| (self.index * FRAME_SAMPLES) as f64 / f64::from(RATE))
    }

    /// Mixes and encodes the next frame.
    pub(crate) fn next_frame(&mut self) -> Result<Vec<u8>, String> {
        let mut mixed = vec![0.0f32; FRAME_SAMPLES * 2];
        self.mixer.render(&mut mixed);
        let pcm: Vec<i16> = mixed
            .iter()
            .map(|s| (s * 32767.0).round().clamp(-32768.0, 32767.0) as i16)
            .collect();
        self.index += 1;
        self.encoder
            .encode_frame(&pcm)
            .map_err(|e| format!("Cannot encode the sound: {e}"))
    }
}
