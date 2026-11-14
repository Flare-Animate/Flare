//! File > Import > Import Video: a movie brought in to draw over. ffprobe is asked what it is,
//! the Import Video dialog says what the choices make before anything is read (the frames, the
//! animation's length at the document's rate, the memory and whether it fits), and the pictures
//! are read on the video worker while a progress dialog can stop them. What is made and the
//! checks are `flare_tools::video`'s, the same the command line and the MCP server use.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use flare_runtime::{VideoJob, VideoOutcome, VideoProgress};
use flare_tools::video::{Plan, Settings, Source};

use super::*;

/// Where an import is.
#[derive(Default)]
pub(super) enum VideoImport {
    #[default]
    Idle,
    /// ffprobe is being asked about a movie.
    Probing { id: u64 },
    /// The dialog is open.
    Asking(Box<VideoForm>),
    /// The pictures are being read.
    Reading {
        id: u64,
        name: String,
        total: u32,
        progress: Arc<VideoProgress>,
    },
}

/// What the Import Video dialog holds while it is open.
pub(super) struct VideoForm {
    pub(super) path: PathBuf,
    /// The file's name without its ending: the layer's and the symbol's.
    pub(super) name: String,
    pub(super) source: Source,
    pub(super) settings: Settings,
}

impl VideoForm {
    /// What the dialog's choices make in `document`; `None` only for choices the dialog's own
    /// ranges do not allow.
    pub(super) fn plan(&self, document: &flare_document::Document) -> Option<Plan> {
        flare_tools::video::plan(document, &self.source, &self.settings).ok()
    }
}

impl Workspace {
    /// Starts bringing in the movie at `path`: ffprobe is asked what it is on the video worker,
    /// and the dialog opens with the answer.
    pub fn import_video(&mut self, path: &Path) {
        if self.in_symbol() {
            self.report(tr!(self, "video-leave-symbol"));
            return;
        }
        if !matches!(self.video_import, VideoImport::Idle) {
            return;
        }
        let id = self.background.next_id();
        self.video_import = VideoImport::Probing { id };
        self.background.submit_video(VideoJob::Probe {
            id,
            path: path.to_owned(),
        });
        if self.background.mode() == flare_runtime::Mode::Threads {
            let name = file_name(path);
            self.status = Some(tr!(self, "video-probing", &[("name", &name)]));
        }
        self.settle_inline();
    }

    pub(super) fn video_probed(&mut self, id: u64, path: PathBuf, result: Result<Source, String>) {
        if !matches!(self.video_import, VideoImport::Probing { id: pending } if pending == id) {
            return;
        }
        self.video_import = VideoImport::Idle;
        match result {
            Ok(source) => {
                let settings = Settings::suggested(self.session.document(), &source);
                self.status = None;
                let name = file_stem(&path).unwrap_or_else(|| tr!(self, "video-layer-name"));
                self.video_import = VideoImport::Asking(Box::new(VideoForm {
                    name,
                    path,
                    source,
                    settings,
                }));
            }
            Err(error) => {
                let message = tr!(self, "video-error", &[("error", &error)]);
                self.report(message);
            }
        }
    }

    /// Reads what the open dialog describes. A choice that does not fit keeps the dialog open.
    pub(super) fn confirm_video_import(&mut self) {
        let VideoImport::Asking(form) = std::mem::take(&mut self.video_import) else {
            return;
        };
        let (Some(plan), Some(scene)) = (form.plan(self.session.document()), self.scene_id())
        else {
            self.video_import = VideoImport::Asking(form);
            return;
        };
        if !plan.fits() {
            self.video_import = VideoImport::Asking(form);
            return;
        }
        let id = self.background.next_id();
        let progress = Arc::new(VideoProgress::default());
        self.video_import = VideoImport::Reading {
            id,
            name: form.name.clone(),
            total: plan.frames,
            progress: Arc::clone(&progress),
        };
        self.background.submit_video(VideoJob::Read {
            id,
            path: form.path,
            document: self.session.snapshot(),
            scene,
            frame: self.frame,
            name: form.name,
            plan: Box::new(plan),
            progress,
        });
        self.settle_inline();
    }

    /// Asks the reading under way to stop.
    pub(super) fn stop_video_import(&mut self) {
        if let VideoImport::Reading { progress, .. } = &self.video_import {
            progress.stop();
        }
    }

    pub(super) fn video_read(&mut self, id: u64, outcome: VideoOutcome) {
        let name = match &self.video_import {
            VideoImport::Reading {
                id: pending, name, ..
            } if *pending == id => name.clone(),
            _ => return,
        };
        self.video_import = VideoImport::Idle;
        let placed = match outcome {
            VideoOutcome::Placed(placed) => placed,
            VideoOutcome::Stopped => {
                self.report(tr!(self, "video-stopped"));
                return;
            }
            VideoOutcome::Failed(error) => {
                let message = tr!(self, "video-error", &[("error", &error)]);
                self.report(message);
                return;
            }
        };
        let label = tr!(self, "video-import");
        let at = placed.at;
        if let Err(error) = self.session.apply_labeled(placed.commands, &label) {
            let message = tr!(self, "video-error", &[("error", &error.to_string())]);
            self.report(message);
            return;
        }
        // Selected with Free Transform, so it is lined up straight away; every picture follows.
        self.current_layer = Some(at.layer);
        self.set_tool(Tool::FreeTransform);
        self.also_selected.clear();
        self.selected = Some(at);
        let message = tr_n!(self, "video-imported", placed.frames, &[("name", &name)]);
        self.report(message);
    }

    /// Whether a video is being probed or read, so another cannot start.
    pub(super) fn video_import_busy(&self) -> bool {
        !matches!(self.video_import, VideoImport::Idle)
    }

    /// The open dialog, for the tests.
    #[cfg(test)]
    pub(super) fn video_form(&mut self) -> Option<&mut VideoForm> {
        match &mut self.video_import {
            VideoImport::Asking(form) => Some(form),
            _ => None,
        }
    }

    /// Pictures read so far and how many are wanted, while reading, for the tests.
    #[cfg(test)]
    pub(super) fn video_reading(&self) -> Option<(u32, u32)> {
        match &self.video_import {
            VideoImport::Reading {
                total, progress, ..
            } => Some((progress.read(), *total)),
            _ => None,
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

fn file_stem(path: &Path) -> Option<String> {
    path.file_stem()
        .map(|n| n.to_string_lossy().trim().to_owned())
        .filter(|n| !n.is_empty())
}
