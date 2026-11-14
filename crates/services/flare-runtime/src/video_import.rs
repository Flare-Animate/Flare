//! Bringing a video into the open document, on a worker of its own: a long read must hold up
//! neither a save nor a redraw. The front end sees how far the reading has got through
//! [`VideoProgress`], and can stop it there.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use flare_core::SceneId;
use flare_document::Document;
use flare_ffmpeg::Tools;
use flare_tools::video::{Placed, Plan};

use crate::jobs::Done;

/// How far a reading has got, and a way to stop it, shared between the worker and the window.
#[derive(Debug, Default)]
pub struct VideoProgress {
    read: AtomicU32,
    stop: AtomicBool,
}

impl VideoProgress {
    /// Pictures read so far.
    pub fn read(&self) -> u32 {
        self.read.load(Ordering::Relaxed)
    }

    /// Asks the reading to stop after the picture it is on.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    pub fn is_stopped(&self) -> bool {
        self.stop.load(Ordering::Relaxed)
    }
}

pub enum VideoJob {
    /// Ask ffprobe what the movie is.
    Probe { id: u64, path: PathBuf },
    /// Read the pictures `plan` asks for and make the commands that place them in `document`,
    /// as a layer called `name` at the bottom of `scene` from `frame`.
    Read {
        id: u64,
        path: PathBuf,
        document: Arc<Document>,
        scene: SceneId,
        frame: u32,
        name: String,
        plan: Box<Plan>,
        progress: Arc<VideoProgress>,
    },
}

/// How a reading ended.
pub enum VideoOutcome {
    /// The commands that place the video, made for the document the job was given.
    Placed(Box<Placed>),
    /// Stopped by the front end.
    Stopped,
    Failed(String),
}

/// Does `job` with the ffmpeg and ffprobe in `tools`, or those on PATH.
pub fn run_video(job: VideoJob, tools: Option<&Tools>) -> Done {
    let found;
    let tools = match tools {
        Some(tools) => Ok(tools),
        None => {
            found = flare_ffmpeg::locate();
            found
                .as_ref()
                .map_err(|_| flare_import::VideoImportError::NoFfmpeg)
        }
    };
    match job {
        VideoJob::Probe { id, path } => Done::VideoProbed {
            id,
            result: tools
                .and_then(|tools| flare_import::probe_video_with(tools, &path))
                .map_err(|e| e.to_string()),
            path,
        },
        VideoJob::Read {
            id,
            path,
            document,
            scene,
            frame,
            name,
            plan,
            progress,
        } => {
            let read = tools.and_then(|tools| {
                flare_import::read_video_with(tools, &path, &plan, |n| {
                    progress.read.store(n, Ordering::Relaxed);
                    !progress.is_stopped()
                })
            });
            let outcome = match read {
                Err(flare_import::VideoImportError::Stopped) => VideoOutcome::Stopped,
                Err(error) => VideoOutcome::Failed(error.to_string()),
                // A stop that came after the last picture still stops the import.
                Ok(_) if progress.is_stopped() => VideoOutcome::Stopped,
                Ok(pictures) => {
                    match flare_tools::video::place(&document, scene, frame, &name, &plan, pictures)
                    {
                        Ok(placed) => VideoOutcome::Placed(Box::new(placed)),
                        Err(error) => VideoOutcome::Failed(error.to_string()),
                    }
                }
            };
            Done::VideoRead { id, outcome }
        }
    }
}
