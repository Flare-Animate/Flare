//! The worker threads: one loop per kind of job, each waking the front end when it finishes.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender};

use crate::jobs::{AudioJob, Done, IoJob, RenderJob, TextJob};
use crate::run::{ExtraFonts, run_audio, run_io, run_render, run_text};

/// Called after each result is queued, so the front end can wake up to show it.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Drawing keeps only the newest request: while a frame is being drawn, any number of newer ones
/// (the user is zooming or scrubbing) collapse into the last.
pub(crate) fn render_loop(jobs: &Receiver<RenderJob>, done: &Sender<Done>, wake: &Wake) {
    while let Ok(mut job) = jobs.recv() {
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        if done.send(run_render(&job)).is_err() {
            return;
        }
        wake();
    }
}

pub(crate) fn audio_loop(jobs: &Receiver<AudioJob>, done: &Sender<Done>, wake: &Wake) {
    while let Ok(job) = jobs.recv() {
        if done.send(run_audio(job)).is_err() {
            return;
        }
        wake();
    }
}

/// As with drawing, only the newest request matters.
pub(crate) fn text_loop(
    jobs: &Receiver<TextJob>,
    done: &Sender<Done>,
    wake: &Wake,
    fonts: &ExtraFonts,
) {
    while let Ok(mut job) = jobs.recv() {
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        if done.send(run_text(&job, fonts)).is_err() {
            return;
        }
        wake();
    }
}

/// Saves, opens and autosaves, in the order they were asked for.
pub(crate) fn io_loop(jobs: &Receiver<IoJob>, done: &Sender<Done>, wake: &Wake) {
    while let Ok(job) = jobs.recv() {
        if done.send(run_io(job)).is_err() {
            return;
        }
        wake();
    }
}
