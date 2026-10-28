//! Sound output: the voices the editor's playback and Test Movie start, mixed and played
//! through the system's default device (tinyaudio), opened on first use.

use std::sync::{Arc, Mutex, MutexGuard};

use minnamate_core::sound::{self, FrameSound, Pcm};

/// The output's sample rate, in stereo.
const RATE: f64 = 44100.0;

/// Whose a voice is: the timeline playing in the editor, Test Movie, or the Library's preview.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Owner {
    Editor,
    Movie,
    Library,
}

/// A sound playing.
pub struct Voice {
    pub owner: Owner,
    /// The library sound's name.
    pub name: String,
    /// Tied to its timeline's frames (a stream sound, or a scrub): it goes when they stop.
    pub stream: bool,
    pcm: Arc<Pcm>,
    /// Where it is, in the sound's samples, and how far each output sample moves it.
    pos: f64,
    step: f64,
    /// The stretch it plays, in the sound's samples, and how many more times.
    from: f64,
    to: f64,
    passes: u32,
    /// Output samples left before it's cut off, if it is.
    left: Option<u64>,
    volumes: Vec<(u32, f32, f32)>,
}

impl Voice {
    /// Keyframe sound `s`, `pcm` decoded, `offset` seconds in and cut off after `limit`
    /// seconds; None if that's past its end.
    pub fn new(owner: Owner, pcm: Arc<Pcm>, s: &FrameSound, offset: f64, limit: Option<f64>) -> Option<Voice> {
        let rate = pcm.rate.max(1) as f64;
        let at = |p: u32| (p as f64 * rate / 44100.0).min(pcm.len() as f64);
        let (from, to) = (at(s.in_point), s.out_point.map_or(pcm.len() as f64, at));
        let passes = if s.looping { u32::MAX } else { s.repeat.max(1) };
        let into = offset.max(0.0) * rate;
        let done = (into / (to - from)) as u32;
        if from >= to || done >= passes {
            return None;
        }
        Some(Voice {
            owner,
            name: s.name.clone(),
            stream: s.sync == sound::Sync::Stream,
            pos: from + into % (to - from),
            step: rate / RATE,
            from,
            to,
            passes: passes - done,
            left: limit.map(|l| (l * RATE).round() as u64),
            volumes: s.volumes((pcm.len() as f64 * 44100.0 / rate) as u32),
            pcm,
        })
    }

    /// Adds what comes next of it to `out` (interleaved stereo); false once it's done.
    fn play(&mut self, out: &mut [f32]) -> bool {
        let rate = self.pcm.rate.max(1) as f64;
        for frame in out.chunks_exact_mut(2) {
            if self.left == Some(0) {
                return false;
            }
            if self.pos >= self.to {
                self.passes -= 1;
                if self.passes == 0 {
                    return false;
                }
                self.pos = self.from + (self.pos - self.to);
            }
            let (i, t) = (self.pos as usize, self.pos.fract() as f32);
            let sample = |c| {
                let (a, b) = (self.pcm.at(i, c) as f32, self.pcm.at(i + 1, c) as f32);
                (a + (b - a) * t) / 32768.0
            };
            let (l, r) = sound::volume(&self.volumes, self.pos * 44100.0 / rate);
            frame[0] += sample(0) * l;
            frame[1] += sample(1) * r;
            self.pos += self.step;
            if let Some(left) = &mut self.left {
                *left -= 1;
            }
        }
        true
    }
}

/// The voices playing, and the device playing them.
#[derive(Default)]
pub struct Player {
    voices: Arc<Mutex<Vec<Voice>>>,
    device: Option<tinyaudio::OutputDevice>,
    /// Why there's no sound, once opening the device failed; it isn't tried again.
    pub failed: Option<String>,
}

impl Player {
    pub fn play(&mut self, voice: Voice) {
        self.open();
        lock(&self.voices).push(voice);
    }

    /// Stops the voices `which` picks.
    pub fn stop(&self, which: impl Fn(&Voice) -> bool) {
        lock(&self.voices).retain(|v| !which(v));
    }

    /// Whether a voice `which` picks is playing.
    pub fn playing(&self, which: impl Fn(&Voice) -> bool) -> bool {
        lock(&self.voices).iter().any(which)
    }

    /// The voices playing: whose, which sound, and whether it streams.
    #[cfg(test)]
    pub fn voices(&self) -> Vec<(Owner, String, bool)> {
        lock(&self.voices).iter().map(|v| (v.owner, v.name.clone(), v.stream)).collect()
    }

    fn open(&mut self) {
        // Tests run silent.
        if self.device.is_some() || self.failed.is_some() || cfg!(test) {
            return;
        }
        let voices = self.voices.clone();
        let params = tinyaudio::OutputDeviceParameters { channels_count: 2, sample_rate: RATE as usize, channel_sample_count: 1024 };
        let device = tinyaudio::run_output_device(params, move |out| {
            out.fill(0.0);
            lock(&voices).retain_mut(|v| v.play(out));
            out.iter_mut().for_each(|s| *s = s.clamp(-1.0, 1.0));
        });
        match device {
            Ok(d) => self.device = Some(d),
            Err(e) => self.failed = Some(e.to_string()),
        }
    }
}

fn lock(voices: &Mutex<Vec<Voice>>) -> MutexGuard<'_, Vec<Voice>> {
    voices.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voices_play_their_stretch_and_repeat() {
        let pcm = Arc::new(Pcm { rate: 44100, channels: 1, samples: vec![8192, 16384, 24576, 32767] });
        let s = FrameSound { in_point: 1, out_point: Some(3), repeat: 2, ..FrameSound::new("a") };
        let mut v = Voice::new(Owner::Editor, pcm.clone(), &s, 0.0, None).unwrap();
        let mut out = vec![0.0; 10];
        assert!(!v.play(&mut out), "four samples, then done");
        let left: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        assert_eq!(left, [0.5, 0.75, 0.5, 0.75, 0.0]);
        // A stream joined part way, and cut off.
        let mut v = Voice::new(Owner::Movie, pcm.clone(), &FrameSound::new("a"), 2.0 / 44100.0, Some(1.0 / 44100.0)).unwrap();
        let mut out = vec![0.0; 4];
        assert!(!v.play(&mut out));
        assert_eq!(out, [0.75, 0.75, 0.0, 0.0]);
        assert!(Voice::new(Owner::Editor, pcm, &FrameSound::new("a"), 1.0, None).is_none(), "past its end");
    }
}
