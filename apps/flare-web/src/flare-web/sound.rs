//! The browser's sound card: the workspace's [`AudioEvent`]s carried out with Web Audio, as
//! `flare-platform`'s player does on the desktop.

use std::sync::Arc;

use flare_runtime::{Audio, AudioEvent, Excerpt};
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext};

/// The most samples the browser copies of sounds may hold together (64 MiB of floats). A scrub
/// asks for a sound many times a second, and copying all of it into a new buffer each time made
/// dragging the playhead over a long sound stall the page.
const KEPT_SAMPLES: usize = 16 * 1024 * 1024;

/// Seconds over which a slice fades in and out, so a scrub does not click.
const EDGE_FADE: f64 = 0.0015;

#[derive(Default)]
pub(crate) struct BrowserSound {
    context: Option<AudioContext>,
    playing: Vec<AudioBufferSourceNode>,
    /// Sounds already copied into the browser, oldest first. The `Arc` is held so the sound's
    /// address, by which it is found again, cannot be reused by another while it is kept.
    kept: Vec<(Arc<Audio>, AudioBuffer)>,
}

impl BrowserSound {
    /// Carries out `events`. The error is for the person: the browser refused to play.
    pub(crate) fn apply(&mut self, events: Vec<AudioEvent>) -> Result<(), String> {
        if events.is_empty() {
            return Ok(());
        }
        if self.context.is_none() {
            let context =
                AudioContext::new().map_err(|e| format!("Cannot start browser audio: {e:?}"))?;
            self.context = Some(context);
        }
        let Some(context) = self.context.clone() else {
            return Ok(());
        };
        let _ = context.resume();
        for event in events {
            match event {
                AudioEvent::Stop => {
                    for source in self.playing.drain(..) {
                        if let Some(scheduled) =
                            source.dyn_ref::<web_sys::AudioScheduledSourceNode>()
                        {
                            let _ = scheduled.stop_with_when(context.current_time());
                        }
                    }
                }
                AudioEvent::Play {
                    audio,
                    delay_seconds,
                    excerpt,
                } => self
                    .play(&context, audio, delay_seconds, excerpt)
                    .map_err(|e| format!("Cannot play sound in browser: {e:?}"))?,
            }
        }
        Ok(())
    }

    fn play(
        &mut self,
        context: &AudioContext,
        audio: Arc<Audio>,
        delay_seconds: f64,
        excerpt: Excerpt,
    ) -> Result<(), JsValue> {
        let range = excerpt.frames(audio.sample_rate(), audio.frames());
        if range.is_empty() {
            return Ok(());
        }
        let rate = f64::from(audio.sample_rate());
        let (offset, duration) = (range.start as f64 / rate, range.len() as f64 / rate);
        let buffer = self.buffer(context, audio.clone())?;
        let source = context.create_buffer_source()?;
        source.set_buffer(Some(&buffer));
        let when = context.current_time() + delay_seconds.max(0.0);
        if excerpt == Excerpt::WHOLE {
            source.connect_with_audio_node(&context.destination())?;
            source.start_with_when(when)?;
        } else {
            // A slice cut out of the sound fades at its ends, as the desktop mixer's does.
            let gain = context.create_gain()?;
            let level = gain.gain();
            let fade = EDGE_FADE.min(duration / 2.0);
            level.set_value_at_time(0.0, when)?;
            level.linear_ramp_to_value_at_time(1.0, when + fade)?;
            level.set_value_at_time(1.0, when + duration - fade)?;
            level.linear_ramp_to_value_at_time(0.0, when + duration)?;
            source.connect_with_audio_node(&gain)?;
            gain.connect_with_audio_node(&context.destination())?;
            source.start_with_when_and_grain_offset_and_grain_duration(when, offset, duration)?;
        }
        self.playing.push(source);
        Ok(())
    }

    /// The browser's copy of `audio`, made the first time it is played.
    fn buffer(
        &mut self,
        context: &AudioContext,
        audio: Arc<Audio>,
    ) -> Result<AudioBuffer, JsValue> {
        if let Some((_, buffer)) = self.kept.iter().find(|(kept, _)| Arc::ptr_eq(kept, &audio)) {
            return Ok(buffer.clone());
        }
        let channels = audio.channels().min(2);
        let buffer = context.create_buffer(
            u32::from(channels),
            audio.frames() as u32,
            audio.sample_rate() as f32,
        )?;
        for channel in 0..channels {
            let samples: Vec<f32> = audio
                .samples()
                .chunks_exact(usize::from(audio.channels()))
                .map(|frame| frame[usize::from(channel)])
                .collect();
            buffer.copy_to_channel_with_f32_array(
                &js_sys::Float32Array::from(samples.as_slice()),
                i32::from(channel),
            )?;
        }
        self.kept.push((audio, buffer.clone()));
        let mut total: usize = self.kept.iter().map(|(a, _)| a.samples_len()).sum();
        while total > KEPT_SAMPLES && self.kept.len() > 1 {
            let (oldest, _) = self.kept.remove(0);
            total -= oldest.samples_len();
        }
        Ok(buffer)
    }
}
