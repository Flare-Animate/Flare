//! The Raster Brush: painting a color into a bitmap's pixels, blended with the colors already
//! there as far as its Color mix says, as Clip Studio Paint's brushes with color mixing do.
//!
//! It paints on one layer, as the other drawing tools do: into the topmost bitmap of that layer's
//! keyframe under the start of the stroke, or, with none there, into a new clear bitmap the size
//! of the stage, which its first edit places on the layer (making a keyframe or a layer as
//! drawing a shape would). So a second stroke over the first paints into the same picture and
//! mixes with it.
//!
//! The brush carries the color it picks up as Smudge does, a running mix of what was under each
//! dab before the stroke began, and each dab lays its paint moved towards that color by the mix.
//! Clear pixels have no color to give: the less paint is under the brush, the less it mixes.

use std::sync::Arc;

use flare_commands::Command;
use flare_core::{BitmapId, InstanceId, LayerId, Rgba8, SceneId, Transform2D};
use flare_document::{BitmapInstance, Content, Document, MAX_BITMAP_PIXELS, MAX_BITMAP_SIDE};
use glamx::{DAffine2, DVec2};

use super::canvas::{Canvas, Premul, lerp};
use super::{
    BlendEdit, BlendError, BlendKind, BlendSettings, BlendStroke, InputSample, MAX_POINTS,
    in_library,
};
use crate::common::{SpanRef, scene_timeline};
use crate::drawing::{DrawRefusal, Target, place_contents};
use crate::magic_wand::{BitmapHit, Pos2, pixel_under};

/// How many pixels a new painting has to a stage unit, across and down. Two keep a stroke sharp
/// on a high-density screen at 100% and soft-edged only past 200%, where one pixel a unit showed
/// each pixel as a block three or four screen pixels wide. A stage too large for that gets one.
pub const PAINT_DENSITY: u32 = 2;

/// What the Raster Brush paints with, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct RasterPaint {
    /// The paint, straight alpha; its alpha is how opaque the paint is.
    pub color: Rgba8,
    /// The layer painted on.
    pub layer: Target,
    /// The name of the layer made when the scene has none that can take art.
    pub new_layer: String,
    /// The name a new bitmap gets in the library.
    pub bitmap_name: String,
}

/// The brush's paint and the color it has picked up.
pub(super) struct Painting {
    /// Straight color, 0 to 1 a channel.
    paint: [f32; 4],
    /// The running mix of what was under the brush, premultiplied; `None` before any dab was on
    /// the bitmap.
    picked: Option<Premul>,
}

impl Painting {
    pub(super) fn new(color: Rgba8) -> Self {
        let [r, g, b, a] = [color.r, color.g, color.b, color.a].map(|c| f32::from(c) / 255.0);
        Self {
            paint: [r, g, b, a],
            picked: None,
        }
    }

    /// The color the next dab lays, premultiplied, given what is under it (`None` off the
    /// bitmap), the mix, and how quickly the picked-up color follows what is under the brush.
    pub(super) fn next_color(&mut self, under: Option<Premul>, mix: f32, keep: f32) -> Premul {
        if let Some(under) = under {
            self.picked = Some(match self.picked {
                Some(picked) => lerp(picked, under, keep),
                None => under,
            });
        }
        let [r, g, b, a] = self.paint;
        let mut rgb = [r, g, b];
        if let Some(picked) = self.picked
            && picked[3] > 1e-4
        {
            // The picked-up color counts for as much paint as it holds: clear pixels give none.
            let t = mix.clamp(0.0, 1.0) * picked[3].min(1.0);
            for (c, p) in rgb.iter_mut().zip(picked) {
                *c += ((p / picked[3]).clamp(0.0, 1.0) - *c) * t;
            }
        }
        [rgb[0] * a, rgb[1] * a, rgb[2] * a, a]
    }
}

/// Where a new bitmap goes when the stroke's first edit is made.
pub(super) struct Unplaced {
    layer: Target,
    new_layer: String,
    frame: u32,
}

impl BlendStroke {
    /// Starts a Raster Brush stroke at `start` on `paint`'s layer at `frame`: on the topmost
    /// bitmap of the layer's keyframe under `start`, or on a new clear bitmap the size of the
    /// stage when there is none. A layer that cannot be drawn on is refused, as drawing a shape
    /// on it would be. Lays no dab yet.
    pub fn paint(
        document: &Document,
        scene: SceneId,
        frame: u32,
        start: Pos2,
        settings: BlendSettings,
        paint: RasterPaint,
    ) -> Result<Self, BlendError> {
        settings.validate()?;
        let layer = paint_layer(document, scene, paint.layer, frame, &paint.new_layer)
            .map_err(BlendError::Refused)?;
        let hit = layer.and_then(|layer| bitmap_on_layer(document, scene, layer, frame, start));
        let mut stroke = match hit {
            Some(hit) => Self::on_bitmap(document, scene, &hit, BlendKind::RasterBrush, settings)?,
            None => Self::blank(document, settings, &paint, frame)?,
        };
        stroke.painting = Some(Painting::new(paint.color));
        Ok(stroke)
    }

    /// A stroke on a new clear bitmap covering the stage, [`PAINT_DENSITY`] pixels to a stage
    /// unit where the bitmap may be that large, else one.
    fn blank(
        document: &Document,
        settings: BlendSettings,
        paint: &RasterPaint,
        frame: u32,
    ) -> Result<Self, BlendError> {
        let stage = document.stage();
        let fits = |density: u32| {
            let (w, h) = (stage.width() * density, stage.height() * density);
            w <= MAX_BITMAP_SIDE
                && h <= MAX_BITMAP_SIDE
                && u64::from(w) * u64::from(h) <= MAX_BITMAP_PIXELS
        };
        let density = [PAINT_DENSITY, 1]
            .into_iter()
            .find(|d| fits(*d))
            .ok_or(BlendError::StageTooLarge(stage.width(), stage.height()))?;
        let (width, height) = (stage.width() * density, stage.height() * density);
        let pixels = u64::from(width) * u64::from(height);
        let clear: Arc<[u8]> = vec![0u8; pixels as usize * 4].into();
        // Nothing is at this place until the first edit puts the bitmap on the layer.
        let nowhere = SpanRef {
            layer: LayerId::new(),
            start: frame,
            element: 0,
        };
        let mut stroke = Self::new(
            BlendKind::RasterBrush,
            settings,
            nowhere,
            None,
            paint.bitmap_name.clone(),
            DAffine2::from_scale(DVec2::splat(f64::from(density))),
            density as f32,
            Canvas::new(width, height, clear, None),
        );
        stroke.unplaced = Some(Unplaced {
            layer: paint.layer,
            new_layer: paint.new_layer.clone(),
            frame,
        });
        Ok(stroke)
    }

    /// The first edit of a stroke on a new bitmap: the bitmap in the library and on the layer.
    pub(super) fn place(
        &self,
        document: &Document,
        scene: SceneId,
        unplaced: &Unplaced,
    ) -> Result<BlendEdit, BlendError> {
        if !self.canvas.dirty {
            return Err(BlendError::NothingChanged);
        }
        let mut commands = Vec::new();
        let id = in_library(document, self.made()?, &mut commands);
        // Back from the painting's pixels to stage units: a dense painting is drawn smaller.
        let transform = Transform2D::new(self.to_pixels.inverse())
            .map_err(|_| BlendError::Bitmap("the painting's scale is not finite".into()))?;
        let placed = BitmapInstance::new(InstanceId::new(), id, transform);
        let plan = place_contents(
            document,
            scene,
            unplaced.layer,
            unplaced.frame,
            None,
            vec![Content::Bitmap(placed)],
            &unplaced.new_layer,
        )
        .map_err(BlendError::Refused)?;
        let at = plan.placed.first().copied().ok_or(BlendError::NotBitmap)?;
        commands.extend(plan.commands);
        Ok(BlendEdit {
            commands,
            bitmap: id,
            at,
        })
    }
}

/// Whether the Raster Brush could paint on `layer` at `frame`: refused as drawing a shape there
/// would be (locked, hidden, not a layer for art, a keyframe that holds a tween). A front end
/// asks before the stroke starts, to say why in its own words.
pub fn check_paint(
    document: &Document,
    scene: SceneId,
    layer: Target,
    frame: u32,
) -> Result<(), DrawRefusal> {
    paint_layer(document, scene, layer, frame, "").map(|_| ())
}

/// A whole Raster Brush stroke through `points` at once, as a front end with no pointer gives
/// it: the commands that paint it.
pub fn lay_paint(
    document: &Document,
    scene: SceneId,
    frame: u32,
    settings: BlendSettings,
    paint: RasterPaint,
    points: &[InputSample],
) -> Result<BlendEdit, BlendError> {
    let first = points.first().ok_or(BlendError::NoPoints)?;
    if points.len() > MAX_POINTS {
        return Err(BlendError::TooManyPoints);
    }
    let start = Pos2::new(first.x as f32, first.y as f32);
    let mut stroke = BlendStroke::paint(document, scene, frame, start, settings, paint)?;
    stroke.extend(points);
    if !stroke.changed() {
        return Err(BlendError::NothingChanged);
    }
    stroke.edit(document, scene)
}

/// The layer a drawing on `target` at `frame` goes to, `None` when one would have to be made;
/// refused as drawing there would be.
fn paint_layer(
    document: &Document,
    scene: SceneId,
    target: Target,
    frame: u32,
    new_layer: &str,
) -> Result<Option<LayerId>, DrawRefusal> {
    let probe = BitmapInstance::new(InstanceId::new(), BitmapId::new(), Transform2D::IDENTITY);
    let plan = place_contents(
        document,
        scene,
        target,
        frame,
        None,
        vec![Content::Bitmap(probe)],
        new_layer,
    )?;
    let made = plan
        .commands
        .iter()
        .any(|c| matches!(c, Command::AddLayer { .. }));
    Ok((!made).then_some(plan.layer))
}

/// The topmost bitmap of `layer`'s keyframe at `frame` with a pixel under `point`.
pub(super) fn bitmap_on_layer(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    frame: u32,
    point: Pos2,
) -> Option<BitmapHit> {
    let span = scene_timeline(document, scene)?
        .layers()
        .iter()
        .find(|l| l.id() == layer)?
        .span_at(frame)?;
    let point = DVec2::new(f64::from(point.x), f64::from(point.y));
    let elements: Vec<&Content> = span.elements().collect();
    elements
        .iter()
        .enumerate()
        .rev()
        .find_map(|(element, content)| {
            let Content::Bitmap(placed) = content else {
                return None;
            };
            let bitmap = document.bitmap(placed.bitmap())?;
            let (x, y) = pixel_under(placed, bitmap, point)?;
            Some(BitmapHit {
                at: SpanRef {
                    layer,
                    start: span.start(),
                    element,
                },
                bitmap: placed.bitmap(),
                x,
                y,
            })
        })
}

#[cfg(test)]
#[path = "paint_tests.rs"]
mod tests;
