//! Sounds made ready for the mixer. The mixer plays only sounds at the device's rate and cannot
//! wait, so a sound of another rate is resampled on a worker thread first and then queued; the
//! window thread never does the work. The result is kept for the next time, so only the first
//! play of such a sound starts late (by as long as the resampling takes).

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use flare_audio::{Audio, Excerpt};

use crate::audio::Sender;

/// The most samples the resampled sounds kept for reuse may hold together (64 MiB).
const KEPT_SAMPLES: usize = 16 * 1024 * 1024;

/// A sound and its copy at the device's rate. The original is held so its address, which is how
/// the sound is told apart, cannot be reused by another sound while this is kept.
struct Kept {
    original: Arc<Audio>,
    ready: Arc<Audio>,
}

#[derive(Default)]
struct State {
    kept: VecDeque<Kept>,
    /// Sounds being resampled, and when and what of them each request for them wants to play.
    waiting: HashMap<usize, Vec<(Instant, Excerpt)>>,
}

impl State {
    fn remember(&mut self, original: Arc<Audio>, ready: Arc<Audio>) {
        self.kept.push_back(Kept { original, ready });
        let mut total: usize = self.kept.iter().map(|k| k.ready.samples_len()).sum();
        while total > KEPT_SAMPLES && self.kept.len() > 1 {
            if let Some(oldest) = self.kept.pop_front() {
                total -= oldest.ready.samples_len();
            }
        }
    }
}

/// What the window plays through: sounds at the device's rate go straight to the queue, others
/// wait for their resampled copy.
pub(crate) struct Prepared {
    sender: Arc<Sender>,
    state: Arc<Mutex<State>>,
}

fn key(audio: &Arc<Audio>) -> usize {
    Arc::as_ptr(audio) as usize
}

fn lock(state: &Mutex<State>) -> MutexGuard<'_, State> {
    // A panic while holding the lock leaves nothing half-done worth refusing to read.
    state.lock().unwrap_or_else(|e| e.into_inner())
}

impl Prepared {
    pub(crate) fn new(sender: Arc<Sender>) -> Self {
        Self {
            sender,
            state: Arc::default(),
        }
    }

    /// Starts the `excerpt` of `audio` after `delay_seconds`, once it is ready for the device.
    /// The excerpt is in seconds, so it means the same part of the resampled copy.
    pub(crate) fn play(&self, audio: Arc<Audio>, delay_seconds: f64, excerpt: Excerpt) {
        if audio.sample_rate() == self.sender.rate() {
            self.sender.play_excerpt(audio, delay_seconds, excerpt);
            return;
        }
        let due = Instant::now() + Duration::from_secs_f64(delay_seconds.clamp(0.0, 3600.0));
        let key = key(&audio);
        let mut state = lock(&self.state);
        if let Some(kept) = state.kept.iter().find(|k| key == self::key(&k.original)) {
            let ready = kept.ready.clone();
            drop(state);
            self.sender.play_excerpt(ready, remaining(due), excerpt);
            return;
        }
        if let Some(requests) = state.waiting.get_mut(&key) {
            requests.push((due, excerpt));
            return;
        }
        state.waiting.insert(key, vec![(due, excerpt)]);
        drop(state);
        let (sender, shared) = (self.sender.clone(), self.state.clone());
        let started = std::thread::Builder::new()
            .name("flare-resample".into())
            .spawn(move || {
                let ready = audio.resampled(sender.rate()).map(Arc::new);
                let mut state = lock(&shared);
                let requests = state.waiting.remove(&key).unwrap_or_default();
                let Some(ready) = ready else { return };
                state.remember(audio, ready.clone());
                drop(state);
                for (due, excerpt) in requests {
                    sender.play_excerpt(ready.clone(), remaining(due), excerpt);
                }
            });
        if started.is_err() {
            lock(&self.state).waiting.remove(&key);
        }
    }

    /// Silences what plays and forgets the plays still waiting for their resampling.
    pub(crate) fn stop_all(&self) {
        for requests in lock(&self.state).waiting.values_mut() {
            requests.clear();
        }
        self.sender.stop_all();
    }
}

fn remaining(due: Instant) -> f64 {
    due.saturating_duration_since(Instant::now()).as_secs_f64()
}

#[cfg(test)]
#[path = "audio_prepared_tests.rs"]
mod tests;
