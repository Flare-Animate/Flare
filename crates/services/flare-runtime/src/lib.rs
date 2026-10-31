//! Work that must not run on the UI thread: drawing frames, decoding sound, shaping text, and
//! reading and writing files.
//!
//! A front end hands a job an immutable snapshot of the document (cheap to take, see
//! `EditorSession::snapshot`) and gets a result back later through [`Runtime::poll`]. It never
//! waits for a job. Four workers do the work, so a slow save cannot hold up a redraw:
//!
//! - the **render worker** keeps only the newest request;
//! - the **I/O worker** runs saves, opens and autosaves in the order they were asked for;
//! - the **audio** and **text** workers decode and shape, each on a thread of its own because
//!   the first use of either takes a while.
//!
//! In [`Mode::Inline`] a job runs on the calling thread and its result is queued exactly as a
//! worker's would be, so everything above this crate follows one code path and headless tests
//! stay deterministic.
#![forbid(unsafe_code)]

mod export;
mod jobs;
mod movie;
mod run;
mod workers;

pub use export::{export_png, png_bytes};
pub use jobs::{AudioJob, Done, IoJob, RenderJob, Rendered, TextJob};
pub use movie::{MAX_MOVIE_FRAMES, MAX_MOVIE_SIDE, gif_bytes};
pub use run::{ExtraFonts, run_audio, run_io, run_render, run_text};
pub use workers::Wake;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Jobs run on the calling thread. Used by tests, and until a window exists.
    Inline,
    /// Jobs run on worker threads.
    Threads,
}

pub struct Runtime {
    mode: Mode,
    next_id: u64,
    done_tx: Sender<Done>,
    done_rx: Receiver<Done>,
    workers: Option<Workers>,
    /// Saves, opens and autosaves handed over and not yet reported back.
    io_in_flight: usize,
    fonts: ExtraFonts,
}

struct Workers {
    render_tx: Option<Sender<RenderJob>>,
    io_tx: Option<Sender<IoJob>>,
    text_tx: Option<Sender<TextJob>>,
    audio_tx: Option<Sender<AudioJob>>,
    handles: Vec<JoinHandle<()>>,
}

impl Runtime {
    pub fn inline() -> Self {
        let (done_tx, done_rx) = channel();
        Self {
            mode: Mode::Inline,
            next_id: 1,
            done_tx,
            done_rx,
            workers: None,
            io_in_flight: 0,
            fonts: Arc::default(),
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Makes `font` (the bytes of a font file) known by name to text jobs, before the first one
    /// runs. Tests use this so they do not depend on the fonts a machine has installed.
    pub fn add_font(&mut self, font: Vec<u8>) {
        Arc::make_mut(&mut self.fonts).push(font);
    }

    /// Starts the worker threads. `wake` is called whenever a result is ready, so the front end
    /// can wake up to show it. Calling this again does nothing.
    pub fn start_threads(&mut self, wake: Wake) {
        if self.workers.is_some() {
            return;
        }
        let (render_tx, render_rx) = channel::<RenderJob>();
        let (io_tx, io_rx) = channel::<IoJob>();
        let (text_tx, text_rx) = channel::<TextJob>();
        let (audio_tx, audio_rx) = channel::<AudioJob>();
        let spawn = |name: &str, body: Box<dyn FnOnce() + Send>| {
            std::thread::Builder::new().name(name.into()).spawn(body)
        };
        let render = {
            let (done, wake) = (self.done_tx.clone(), Arc::clone(&wake));
            spawn(
                "flare-render",
                Box::new(move || workers::render_loop(&render_rx, &done, &wake)),
            )
        };
        let io = {
            let (done, wake) = (self.done_tx.clone(), Arc::clone(&wake));
            spawn(
                "flare-io",
                Box::new(move || workers::io_loop(&io_rx, &done, &wake)),
            )
        };
        let audio = {
            let (done, wake) = (self.done_tx.clone(), Arc::clone(&wake));
            spawn(
                "flare-audio",
                Box::new(move || workers::audio_loop(&audio_rx, &done, &wake)),
            )
        };
        let text = {
            let (done, fonts) = (self.done_tx.clone(), Arc::clone(&self.fonts));
            spawn(
                "flare-text",
                Box::new(move || workers::text_loop(&text_rx, &done, &wake, &fonts)),
            )
        };
        // If a thread cannot be started, stay inline rather than lose the work.
        let (Ok(render), Ok(io), Ok(text), Ok(audio)) = (render, io, text, audio) else {
            return;
        };
        self.workers = Some(Workers {
            render_tx: Some(render_tx),
            io_tx: Some(io_tx),
            text_tx: Some(text_tx),
            audio_tx: Some(audio_tx),
            handles: vec![render, io, text, audio],
        });
        self.mode = Mode::Threads;
    }

    /// A fresh job number. Numbers are never reused, so a late result is never mistaken for a
    /// newer one's.
    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn submit_render(&self, job: RenderJob) {
        match self.workers.as_ref().and_then(|w| w.render_tx.as_ref()) {
            Some(tx) => {
                // A send fails only if the worker is gone; draw here instead.
                if let Err(failed) = tx.send(job) {
                    let _ = self.done_tx.send(run_render(&failed.0));
                }
            }
            None => {
                let _ = self.done_tx.send(run_render(&job));
            }
        }
    }

    pub fn submit_audio(&self, job: AudioJob) {
        match self.workers.as_ref().and_then(|w| w.audio_tx.as_ref()) {
            Some(tx) => {
                if let Err(failed) = tx.send(job) {
                    let _ = self.done_tx.send(run_audio(failed.0));
                }
            }
            None => {
                let _ = self.done_tx.send(run_audio(job));
            }
        }
    }

    pub fn submit_text(&self, job: TextJob) {
        match self.workers.as_ref().and_then(|w| w.text_tx.as_ref()) {
            Some(tx) => {
                if let Err(failed) = tx.send(job) {
                    let _ = self.done_tx.send(run_text(&failed.0, &self.fonts));
                }
            }
            None => {
                let _ = self.done_tx.send(run_text(&job, &self.fonts));
            }
        }
    }

    pub fn submit_io(&mut self, job: IoJob) {
        self.io_in_flight += 1;
        match self.workers.as_ref().and_then(|w| w.io_tx.as_ref()) {
            Some(tx) => {
                if let Err(failed) = tx.send(job) {
                    let _ = self.done_tx.send(run_io(failed.0));
                }
            }
            None => {
                let _ = self.done_tx.send(run_io(job));
            }
        }
    }

    /// Results that have arrived, oldest first. Never blocks.
    pub fn poll(&mut self) -> Vec<Done> {
        let done: Vec<Done> = self.done_rx.try_iter().collect();
        for finished in &done {
            if finished.is_io() {
                self.io_in_flight = self.io_in_flight.saturating_sub(1);
            }
        }
        done
    }

    /// Whether a save, open or autosave has not been reported back yet.
    pub fn io_pending(&self) -> bool {
        self.io_in_flight > 0
    }

    /// Blocks until every I/O job handed over so far has finished, returning their results.
    /// For shutting down only: a front end otherwise never waits.
    pub fn finish_io(&mut self) -> Vec<Done> {
        let mut all = Vec::new();
        while self.io_in_flight > 0 {
            match self.done_rx.recv() {
                Ok(done) => {
                    if done.is_io() {
                        self.io_in_flight -= 1;
                    }
                    all.push(done);
                }
                Err(_) => break,
            }
        }
        all
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        // Closing the channels lets each worker finish its current job and exit; waiting for
        // them means a save in progress is not cut off when the app closes.
        if let Some(mut workers) = self.workers.take() {
            workers.render_tx = None;
            workers.io_tx = None;
            workers.text_tx = None;
            workers.audio_tx = None;
            for handle in workers.handles.drain(..) {
                let _ = handle.join();
            }
        }
    }
}

#[cfg(test)]
mod tests;
