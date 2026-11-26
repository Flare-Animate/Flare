//! The project's stop-motion session, edited: the one place that decides how a session is started,
//! how a captured picture joins it, how holds change and how it is laid on a timeline, so the Stop
//! Motion panel, `flare stop-motion-*` and the MCP server do the same thing and every change is
//! one undo step. The pictures are library bitmaps, so a picture that is already in the library
//! (the same pixels) is shared, never stored twice.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, MAX_STOP_MOTION_FRAMES, StopFrame,
    StopMotion, StopMotionError,
};

/// The longest timeline a session may be laid on, in frames.
pub const MAX_TIMELINE_FRAMES: u64 = 100_000;

/// Why a session could not be changed.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum StopMotionLibraryError {
    #[error(transparent)]
    Session(#[from] StopMotionError),
    #[error("the project already has a stop-motion session")]
    AlreadyStarted,
    #[error("the project has no stop-motion session")]
    NotStarted,
    #[error("the session has no frame {index} (it has {len})")]
    NoSuchFrame { index: usize, len: usize },
    #[error("the session has no frames to lay on the timeline")]
    NoFrames,
    #[error("the session is {frames} timeline frames long; the most is {MAX_TIMELINE_FRAMES}")]
    TimelineTooLong { frames: u64 },
    #[error("there is no such scene")]
    NoSuchScene,
    #[error("the layer could not be made")]
    Layer,
}

fn session(document: &Document) -> Result<&StopMotion, StopMotionLibraryError> {
    document
        .stop_motion()
        .ok_or(StopMotionLibraryError::NotStarted)
}

fn set(session: StopMotion) -> Command {
    Command::SetStopMotion(Some(session))
}

/// The command that starts a session at `fps` captures a second, each new frame held `hold`.
pub fn start(document: &Document, fps: u32, hold: u32) -> Result<Command, StopMotionLibraryError> {
    if document.stop_motion().is_some() {
        return Err(StopMotionLibraryError::AlreadyStarted);
    }
    Ok(set(StopMotion::new(fps, hold)?))
}

/// The command that ends the session. The pictures stay in the library.
pub fn end(document: &Document) -> Result<Command, StopMotionLibraryError> {
    session(document)?;
    Ok(Command::SetStopMotion(None))
}

/// The commands that append `picture` as the next frame, held `hold` frames (the session's own
/// hold when `None`). A picture with the same pixels as one in the library is shared.
pub fn add_frame(
    document: &Document,
    picture: Bitmap,
    hold: Option<u32>,
) -> Result<Vec<Command>, StopMotionLibraryError> {
    let current = session(document)?;
    let hold = hold.unwrap_or_else(|| current.default_hold());
    let mut commands = Vec::with_capacity(2);
    let id = match document
        .bitmaps()
        .iter()
        .find(|b| b.content_hash() == picture.content_hash())
    {
        Some(same) => same.id(),
        None => {
            let id = picture.id();
            commands.push(Command::AddBitmap(picture));
            id
        }
    };
    let mut frames = current.frames().to_vec();
    if frames.len() >= MAX_STOP_MOTION_FRAMES {
        return Err(StopMotionError::TooManyFrames.into());
    }
    frames.push(StopFrame::new(id, hold)?);
    commands.push(set(current.clone().with_frames(frames)?));
    Ok(commands)
}

fn at(current: &StopMotion, index: usize) -> Result<(), StopMotionLibraryError> {
    if index < current.frames().len() {
        Ok(())
    } else {
        Err(StopMotionLibraryError::NoSuchFrame {
            index,
            len: current.frames().len(),
        })
    }
}

/// The command that holds frame `index` for `hold` timeline frames.
pub fn set_hold(
    document: &Document,
    index: usize,
    hold: u32,
) -> Result<Command, StopMotionLibraryError> {
    let current = session(document)?;
    at(current, index)?;
    let mut frames = current.frames().to_vec();
    frames[index] = StopFrame::new(frames[index].bitmap(), hold)?;
    Ok(set(current.clone().with_frames(frames)?))
}

/// The command that takes frame `index` out of the session. Its picture stays in the library.
pub fn remove_frame(document: &Document, index: usize) -> Result<Command, StopMotionLibraryError> {
    let current = session(document)?;
    at(current, index)?;
    let mut frames = current.frames().to_vec();
    frames.remove(index);
    Ok(set(current.clone().with_frames(frames)?))
}

/// The command that sets the capture rate and the hold new frames get.
pub fn set_rate(
    document: &Document,
    fps: u32,
    hold: u32,
) -> Result<Command, StopMotionLibraryError> {
    Ok(set(session(document)?.clone().with_rate(fps, hold)?))
}

/// The commands that lay the session on a new layer of `scene`, one keyframe per picture for its
/// hold, one after another from the first frame.
pub fn lay_on_timeline(
    document: &Document,
    scene: SceneId,
    layer_name: &str,
) -> Result<(LayerId, Vec<Command>), StopMotionLibraryError> {
    let current = session(document)?;
    if current.frames().is_empty() {
        return Err(StopMotionLibraryError::NoFrames);
    }
    if !document.scenes().iter().any(|s| s.id() == scene) {
        return Err(StopMotionLibraryError::NoSuchScene);
    }
    let total = current.duration();
    if total > MAX_TIMELINE_FRAMES {
        return Err(StopMotionLibraryError::TimelineTooLong { frames: total });
    }
    let mut start = 0_u32;
    let mut spans = Vec::with_capacity(current.frames().len());
    for frame in current.frames() {
        let hold = NonZeroU32::new(frame.hold()).ok_or(StopMotionError::Hold)?;
        let content = Content::Bitmap(BitmapInstance::new(
            InstanceId::new(),
            frame.bitmap(),
            Transform2D::IDENTITY,
        ));
        spans.push(FrameSpan::new(start, hold, content));
        start += frame.hold();
    }
    let id = LayerId::new();
    let layer = Layer::new(id, layer_name, spans)
        .map_err(|_| StopMotionLibraryError::Layer)?
        .with_hold(true);
    Ok((id, vec![Command::AddLayer { scene, layer }]))
}

#[cfg(test)]
#[path = "stop_motion_library_tests.rs"]
mod tests;
