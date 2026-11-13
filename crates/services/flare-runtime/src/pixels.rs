//! Work on the pixels of bitmaps, on a worker of its own: the Magic Wand's picking and clearing
//! take a quarter of a second on the largest bitmap a document may hold (see PERFORMANCE.md),
//! too long to hold up the window, and a Blur or Smudge drag builds a new bitmap as it goes.

use std::sync::Arc;

use emath::Pos2;
use flare_commands::Command;
use flare_core::SceneId;
use flare_document::Document;
use flare_tools::magic_wand::{self, Combine, Mask, Smoothing, WandSelection, WandSettings};

use crate::blend::{BlendJob, LiveBlend, run_blend};
use crate::jobs::Done;

/// The longest side of the picture of a wand selection drawn over the stage.
pub const WAND_PREVIEW_SIDE: u32 = 1024;

pub enum PixelJob {
    /// A click of the wand at `point` (in the coordinates of the timeline edited) at `frame`,
    /// combined with `current` as `combine` says.
    WandPick {
        id: u64,
        document: Arc<Document>,
        scene: SceneId,
        frame: u32,
        point: Pos2,
        settings: WandSettings,
        combine: Combine,
        current: Option<WandSelection>,
    },
    /// The commands that clear the pixels of `selection` in `document`.
    WandClear {
        id: u64,
        document: Arc<Document>,
        scene: SceneId,
        selection: WandSelection,
        smoothing: Smoothing,
    },
    /// A Blur or Smudge drag.
    Blend(BlendJob),
}

/// What a wand click picked, with a picture of it small enough to draw every frame.
#[derive(Debug, Clone, PartialEq)]
pub struct WandPicked {
    pub selection: WandSelection,
    pub preview: Mask,
}

/// What the pixels worker keeps between jobs: the Blur or Smudge stroke being laid.
#[derive(Default)]
pub(crate) struct Pixels {
    blend: Option<LiveBlend>,
}

impl Pixels {
    /// Runs `job`, giving what the window is to hear (a step of a stroke may give nothing).
    pub(crate) fn run(&mut self, job: PixelJob) -> Vec<Done> {
        match job {
            PixelJob::Blend(job) => run_blend(&mut self.blend, job),
            PixelJob::WandPick {
                id,
                document,
                scene,
                frame,
                point,
                settings,
                combine,
                current,
            } => {
                let result =
                    magic_wand::click(&document, scene, frame, point, &settings, combine, current)
                        .map(|picked| {
                            picked.map(|selection| WandPicked {
                                preview: selection.mask.preview(WAND_PREVIEW_SIDE),
                                selection,
                            })
                        })
                        .map_err(|e| e.to_string());
                vec![Done::WandPicked { id, result }]
            }
            PixelJob::WandClear {
                id,
                document,
                scene,
                selection,
                smoothing,
            } => {
                let result: Result<Vec<Command>, String> =
                    magic_wand::clear(&document, scene, &selection, smoothing)
                        .map_err(|e| e.to_string());
                vec![Done::WandCleared {
                    id,
                    document,
                    result,
                }]
            }
        }
    }
}
