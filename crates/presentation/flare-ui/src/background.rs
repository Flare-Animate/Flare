//! Work that must not run on the UI thread: drawing frames, and reading and writing files.
//!
//! The UI hands a job an immutable snapshot of the document (cheap to take, see
//! `EditorSession::snapshot`) and gets a result back later through [`Background::poll`]. It
//! never waits for a job. Two workers do the work, so a slow save cannot hold up a redraw:
//!
//! - the **render worker** keeps only the newest request. While a frame is being drawn, any
//!   number of newer requests (the user is zooming or scrubbing) collapse into the last one, and
//!   its result is what comes back;
//! - the **I/O worker** runs saves, opens and autosaves in the order they were asked for.
//!
//! In [`Mode::Inline`] a job runs on the calling thread and its result is queued exactly as a
//! worker's would be, so everything above this module follows one code path and headless tests
//! stay deterministic.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread::JoinHandle;

use flare_core::{Rgba8, SceneId};
use flare_document::Document;

/// One frame to draw: the part of the stage in `viewport`, at `size` pixels, with onion-skin
/// ghosts of the given frames beneath it.
pub(crate) struct RenderJob {
    pub id: u64,
    pub document: Arc<Document>,
    pub scene: SceneId,
    pub frame: u32,
    /// (frame, opacity), farthest first.
    pub ghosts: Vec<(u32, f32)>,
    pub viewport: flare_render::Viewport,
    pub size: [u32; 2],
    pub background: Rgba8,
}

/// Text to turn into outlines. Building the font database takes long enough that this has a
/// worker of its own, so a first use never holds up a save or a redraw.
pub(crate) struct TextJob {
    pub id: u64,
    pub text: String,
    pub style: flare_text::TextStyle,
}

pub(crate) enum IoJob {
    Save {
        path: PathBuf,
        document: Arc<Document>,
    },
    Open {
        id: u64,
        path: PathBuf,
        /// Opening a recovery file rather than a project.
        recovery: bool,
    },
    Autosave {
        id: u64,
        dir: PathBuf,
        document: Arc<Document>,
    },
}

/// What a worker reports back. Errors are already text: the UI only shows them.
pub(crate) enum Done {
    Text {
        id: u64,
        result: Result<flare_text::TextOutline, String>,
    },
    Rendered {
        id: u64,
        result: Result<egui::ColorImage, String>,
    },
    Saved {
        path: PathBuf,
        document: Arc<Document>,
        result: Result<(), String>,
    },
    Opened {
        id: u64,
        path: PathBuf,
        recovery: bool,
        result: Result<Document, String>,
    },
    Autosaved {
        id: u64,
        result: Result<PathBuf, String>,
    },
}

impl Done {
    /// Whether this finished a save, open or autosave (what `Background::io_pending` counts).
    fn is_io(&self) -> bool {
        matches!(
            self,
            Done::Saved { .. } | Done::Opened { .. } | Done::Autosaved { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Jobs run on the calling thread. Used by tests, and until a window exists.
    Inline,
    /// Jobs run on worker threads.
    Threads,
}

pub(crate) struct Background {
    mode: Mode,
    next_id: u64,
    done_tx: Sender<Done>,
    done_rx: Receiver<Done>,
    workers: Option<Workers>,
    /// Saves, opens and autosaves handed over and not yet reported back.
    io_in_flight: usize,
}

struct Workers {
    render_tx: Option<Sender<RenderJob>>,
    io_tx: Option<Sender<IoJob>>,
    text_tx: Option<Sender<TextJob>>,
    handles: Vec<JoinHandle<()>>,
}

impl Background {
    pub fn inline() -> Self {
        let (done_tx, done_rx) = channel();
        Self {
            mode: Mode::Inline,
            next_id: 1,
            done_tx,
            done_rx,
            workers: None,
            io_in_flight: 0,
        }
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Starts the worker threads. `ctx` is asked to repaint whenever a result is ready, so the UI
    /// wakes up to show it. Calling this again does nothing.
    pub fn start_threads(&mut self, ctx: egui::Context) {
        if self.workers.is_some() {
            return;
        }
        let (render_tx, render_rx) = channel::<RenderJob>();
        let (io_tx, io_rx) = channel::<IoJob>();
        let (text_tx, text_rx) = channel::<TextJob>();
        let render = {
            let (done, ctx) = (self.done_tx.clone(), ctx.clone());
            std::thread::Builder::new()
                .name("flare-render".into())
                .spawn(move || render_loop(&render_rx, &done, &ctx))
        };
        let io = {
            let (done, ctx) = (self.done_tx.clone(), ctx.clone());
            std::thread::Builder::new()
                .name("flare-io".into())
                .spawn(move || io_loop(&io_rx, &done, &ctx))
        };
        let text = {
            let (done, ctx) = (self.done_tx.clone(), ctx);
            std::thread::Builder::new()
                .name("flare-text".into())
                .spawn(move || text_loop(&text_rx, &done, &ctx))
        };
        // If a thread cannot be started, stay inline rather than lose the work.
        let (Ok(render), Ok(io), Ok(text)) = (render, io, text) else {
            return;
        };
        self.workers = Some(Workers {
            render_tx: Some(render_tx),
            io_tx: Some(io_tx),
            text_tx: Some(text_tx),
            handles: vec![render, io, text],
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

    pub fn submit_text(&self, job: TextJob) {
        match self.workers.as_ref().and_then(|w| w.text_tx.as_ref()) {
            Some(tx) => {
                if let Err(failed) = tx.send(job) {
                    let _ = self.done_tx.send(run_text(&failed.0));
                }
            }
            None => {
                let _ = self.done_tx.send(run_text(&job));
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
    /// For shutting down only: the UI otherwise never waits.
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

impl Drop for Background {
    fn drop(&mut self) {
        // Closing the channels lets each worker finish its current job and exit; waiting for
        // them means a save in progress is not cut off when the app closes.
        if let Some(mut workers) = self.workers.take() {
            workers.render_tx = None;
            workers.io_tx = None;
            workers.text_tx = None;
            for handle in workers.handles.drain(..) {
                let _ = handle.join();
            }
        }
    }
}

fn render_loop(jobs: &Receiver<RenderJob>, done: &Sender<Done>, ctx: &egui::Context) {
    while let Ok(mut job) = jobs.recv() {
        // Only the newest request matters; older ones were overtaken while this one waited.
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        if done.send(run_render(&job)).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

fn text_loop(jobs: &Receiver<TextJob>, done: &Sender<Done>, ctx: &egui::Context) {
    while let Ok(mut job) = jobs.recv() {
        // As with drawing, only the newest request matters.
        while let Ok(newer) = jobs.try_recv() {
            job = newer;
        }
        if done.send(run_text(&job)).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

thread_local! {
    /// The font database of this thread, built the first time text is asked for.
    static TEXT_ENGINE: std::cell::RefCell<Option<flare_text::TextEngine>> =
        const { std::cell::RefCell::new(None) };
}

/// The engine each thread builds on first use. Under test it also knows the Hack font by name, so
/// text tests do not depend on the fonts a machine happens to have.
fn default_text_engine() -> flare_text::TextEngine {
    #[allow(unused_mut)]
    let mut engine = flare_text::TextEngine::new();
    #[cfg(test)]
    engine.register_font(epaint_default_fonts::HACK_REGULAR.to_vec());
    engine
}

pub(crate) fn run_text(job: &TextJob) -> Done {
    let result = TEXT_ENGINE.with(|slot| {
        slot.borrow_mut()
            .get_or_insert_with(default_text_engine)
            .outline(&job.text, &job.style)
            .map_err(|e| e.to_string())
    });
    Done::Text { id: job.id, result }
}

fn io_loop(jobs: &Receiver<IoJob>, done: &Sender<Done>, ctx: &egui::Context) {
    while let Ok(job) = jobs.recv() {
        if done.send(run_io(job)).is_err() {
            return;
        }
        ctx.request_repaint();
    }
}

/// Draws the frame. Shared by the worker and the inline mode.
pub(crate) fn run_render(job: &RenderJob) -> Done {
    Done::Rendered {
        id: job.id,
        result: draw(job),
    }
}

fn draw(job: &RenderJob) -> Result<egui::ColorImage, String> {
    let current = flare_eval::evaluate_frame(&job.document, job.scene, job.frame)
        .map_err(|e| e.to_string())?;
    // A ghost that fails to evaluate is simply left out; the current frame matters.
    let evaluated: Vec<(flare_eval::Frame<'_>, f32)> = job
        .ghosts
        .iter()
        .filter_map(|(frame, opacity)| {
            flare_eval::evaluate_frame(&job.document, job.scene, *frame)
                .ok()
                .map(|e| (e, *opacity))
        })
        .collect();
    let ghosts: Vec<(&flare_eval::Frame<'_>, f32)> =
        evaluated.iter().map(|(e, o)| (e, *o)).collect();
    let [width, height] = job.size;
    let size = [width as usize, height as usize];
    if ghosts.is_empty() {
        // The common case goes straight from the rasterizer's premultiplied pixels to the
        // texture, with no conversion pass either side.
        let image = flare_render::render_viewport_premultiplied(
            &current,
            &job.viewport,
            width,
            height,
            job.background,
        )
        .map_err(|e| e.to_string())?;
        Ok(egui::ColorImage::from_rgba_premultiplied(
            size,
            image.rgba(),
        ))
    } else {
        let image = flare_render::render_onion_viewport(
            &ghosts,
            &current,
            &job.viewport,
            width,
            height,
            job.background,
        )
        .map_err(|e| e.to_string())?;
        Ok(egui::ColorImage::from_rgba_unmultiplied(size, image.rgba()))
    }
}

pub(crate) fn run_io(job: IoJob) -> Done {
    match job {
        IoJob::Save { path, document } => {
            let result = flare_storage::save(&document, &path).map_err(|e| e.to_string());
            Done::Saved {
                path,
                document,
                result,
            }
        }
        IoJob::Open { id, path, recovery } => {
            let result = flare_storage::load(&path).map_err(|e| e.to_string());
            Done::Opened {
                id,
                path,
                recovery,
                result,
            }
        }
        IoJob::Autosave { id, dir, document } => Done::Autosaved {
            id,
            result: flare_storage::save_recovery(&document, &dir).map_err(|e| e.to_string()),
        },
    }
}
