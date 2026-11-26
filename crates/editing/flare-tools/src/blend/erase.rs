//! The Eraser on a raster layer: rubbing pixels out of the painting under the start of the drag,
//! a dab at a time, leaving them clear. Strokes are laid as the Brush's are (each pixel keeps the
//! most any dab took from it), so going over a place again in one stroke takes no more.

use flare_core::{LayerId, SceneId};
use flare_document::Document;

use super::paint::bitmap_on_layer;
use super::{
    BlendEdit, BlendError, BlendKind, BlendSettings, BlendStroke, InputSample, MAX_POINTS,
};
use crate::magic_wand::{Pos2, bitmap_at};

impl BlendStroke {
    /// Starts rubbing out at `start` on `layer` at `frame`: on the topmost picture of the layer's
    /// keyframe under `start`. With `None`, on the topmost picture under it on any shown,
    /// unlocked layer, as Blur finds one. Lays no dab yet.
    pub fn erase(
        document: &Document,
        scene: SceneId,
        frame: u32,
        start: Pos2,
        settings: BlendSettings,
        layer: Option<LayerId>,
    ) -> Result<Self, BlendError> {
        settings.validate()?;
        let hit = match layer {
            Some(layer) => bitmap_on_layer(document, scene, layer, frame, start),
            None => bitmap_at(document, scene, frame, start),
        }
        .ok_or(BlendError::NothingToErase)?;
        Self::on_bitmap(document, scene, &hit, BlendKind::Erase, settings)
    }
}

/// A whole Eraser stroke through `points` at once, as a front end with no pointer gives it: the
/// commands that rub out what it passed over on `layer` (or the topmost picture, with `None`).
pub fn rub_out(
    document: &Document,
    scene: SceneId,
    frame: u32,
    settings: BlendSettings,
    layer: Option<LayerId>,
    points: &[InputSample],
) -> Result<BlendEdit, BlendError> {
    let first = points.first().ok_or(BlendError::NoPoints)?;
    if points.len() > MAX_POINTS {
        return Err(BlendError::TooManyPoints);
    }
    let start = Pos2::new(first.x as f32, first.y as f32);
    let mut stroke = BlendStroke::erase(document, scene, frame, start, settings, layer)?;
    stroke.extend(points);
    if !stroke.changed() {
        return Err(BlendError::NothingChanged);
    }
    stroke.edit(document, scene)
}
