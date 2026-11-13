//! A Blur or Smudge drag on the pixels worker. The window hands over the drag a few samples at a
//! time; the worker lays their dabs and sends back an edit (the commands that show the bitmap as
//! the stroke has left it) whenever the last one has been applied. So a large bitmap, whose edit
//! takes longest to build (see PERFORMANCE.md), shows the stroke less often instead of holding up
//! the window, and a small one shows it every frame.

use std::sync::Arc;

use emath::Pos2;
use flare_core::{BitmapId, SceneId};
use flare_document::{Content, Document};
use flare_input::InputSample;
use flare_tools::blend::{BlendError, BlendKind, BlendSettings, BlendStroke};

use crate::jobs::Done;

pub enum BlendJob {
    /// Starts a stroke at `start` on the bitmap under it at `frame`; one stroke at a time, so
    /// this replaces any other.
    Begin {
        id: u64,
        document: Arc<Document>,
        scene: SceneId,
        frame: u32,
        start: Pos2,
        kind: BlendKind,
        settings: BlendSettings,
    },
    /// More of the drag (perhaps none), with the document as the window has it now. `last` says
    /// the drag has ended: the stroke ends once its last change has been shown.
    Step {
        id: u64,
        document: Arc<Document>,
        samples: Vec<InputSample>,
        last: bool,
    },
    /// Forgets the stroke: it was cancelled, or an edit of it could not be applied.
    Drop { id: u64 },
}

/// The stroke the worker is laying.
pub(crate) struct LiveBlend {
    id: u64,
    scene: SceneId,
    stroke: BlendStroke,
    /// The edit handed back and not yet seen applied: the bitmap it shows, and the one shown
    /// before it.
    awaiting: Option<(BitmapId, BitmapId)>,
    last: bool,
}

/// Runs `job` against the stroke in `live`, giving what the window is to hear.
pub(crate) fn run_blend(live: &mut Option<LiveBlend>, job: BlendJob) -> Vec<Done> {
    match job {
        BlendJob::Begin {
            id,
            document,
            scene,
            frame,
            start,
            kind,
            settings,
        } => match BlendStroke::begin(&document, scene, frame, start, kind, settings) {
            Ok(stroke) => {
                *live = Some(LiveBlend {
                    id,
                    scene,
                    stroke,
                    awaiting: None,
                    last: false,
                });
                Vec::new()
            }
            Err(error) => {
                *live = None;
                vec![ended(id, Some(error))]
            }
        },
        BlendJob::Step {
            id,
            document,
            samples,
            last,
        } => {
            let Some(mut stroke) = live.take().filter(|l| l.id == id) else {
                return Vec::new();
            };
            let out = step(&mut stroke, document, &samples, last);
            if !out.iter().any(|d| matches!(d, Done::BlendEnded { .. })) {
                *live = Some(stroke);
            }
            out
        }
        BlendJob::Drop { id } => {
            if live.as_ref().is_some_and(|l| l.id == id) {
                *live = None;
            }
            Vec::new()
        }
    }
}

fn step(
    live: &mut LiveBlend,
    document: Arc<Document>,
    samples: &[InputSample],
    last: bool,
) -> Vec<Done> {
    live.stroke.extend(samples);
    live.last |= last;
    if let Some((made, before)) = live.awaiting {
        match shown(&document, live.scene, &live.stroke) {
            Some(bitmap) if bitmap == made => live.awaiting = None,
            // Still on its way to the window; it will send the document again once applied.
            Some(bitmap) if bitmap == before => return Vec::new(),
            _ => return vec![ended(live.id, Some(BlendError::Stale))],
        }
    }
    if live.stroke.changed() {
        match live.stroke.edit(&document, live.scene) {
            Ok(edit) => {
                live.awaiting = Some((edit.bitmap, live.stroke.bitmap()));
                live.stroke.shows(edit.bitmap);
                return vec![Done::BlendEdited {
                    id: live.id,
                    document,
                    result: edit,
                }];
            }
            Err(BlendError::NothingChanged) => {
                let bitmap = live.stroke.bitmap();
                live.stroke.shows(bitmap);
            }
            Err(error) => return vec![ended(live.id, Some(error))],
        }
    }
    if live.last {
        return vec![ended(live.id, None)];
    }
    Vec::new()
}

/// The bitmap the stroke's element shows in `document`, if it is still a bitmap.
fn shown(document: &Document, scene: SceneId, stroke: &BlendStroke) -> Option<BitmapId> {
    match flare_tools::ops::find_element(document, scene, stroke.at()) {
        Some(Content::Bitmap(placed)) => Some(placed.bitmap()),
        _ => None,
    }
}

fn ended(id: u64, error: Option<BlendError>) -> Done {
    Done::BlendEnded {
        id,
        error: error.map(|e| e.to_string()),
    }
}

#[cfg(test)]
#[path = "blend_tests.rs"]
mod tests;
