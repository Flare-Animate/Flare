//! Strokes on a bitmap's pixels: Blur and Smudge, which soften a bitmap or push its colors along
//! as Clip Studio Paint's blending tools do, and the Raster Brush, which paints a color into it
//! and mixes in the colors already there. Blur and Smudge work on the bitmap under the start of
//! the stroke; the Raster Brush on the one under it on the layer it paints, or on a new clear
//! bitmap the size of the stage (see `paint`). Each stroke makes a new bitmap for that one
//! element (as a Magic Wand clear does), so other frames that show the same picture keep it as
//! it was.
//!
//! A stroke is a row of round dabs along the drag, spaced by a fraction of their size. Blur moves
//! each pixel a dab covers towards a blurred copy of the bitmap; Smudge picks up the colors under
//! each dab, carries a running mix of them along, and lays it down at the next; the Raster Brush
//! lays its paint, tinted by the running mix as far as its Color mix says. The stroke is capped
//! at its strength (see `canvas`), and pen pressure sets the size of each dab.

mod canvas;
mod dab;
mod paint;
mod settings;

use flare_commands::Command;
use flare_core::{BitmapId, SceneId};
use flare_document::{Bitmap, BitmapInstance, Content, Document};
/// The samples a stroke takes; named here so a caller needs no `flare-input` of its own.
pub use flare_input::{InputSample, PointerKind};
use glamx::{DAffine2, DVec2};

use crate::common::{SpanRef, find_element, replace_element};
use crate::magic_wand::{BitmapHit, Pos2, bitmap_at, uses};
use canvas::{Canvas, Premul};
use dab::Dab;
use paint::{Painting, Unplaced};
pub use paint::{RasterPaint, check_paint, lay_paint};
pub use settings::{BlendError, BlendKind, BlendSettings, MAX_SIZE, MAX_STRETCH};

/// The widest a dab is drawn, in bitmap pixels, however the bitmap is scaled.
pub const MAX_DAB_PIXELS: f32 = 1024.0;
/// Most points one [`rub`] takes.
pub const MAX_POINTS: usize = 4096;
/// Most dabs one stroke lays; past this the stroke stops changing the bitmap.
pub const MAX_DABS: usize = 200_000;

/// What a stroke's commands put on the stage: apply `commands`, then tell the stroke with
/// [`BlendStroke::shows`].
#[derive(Debug, Clone, PartialEq)]
pub struct BlendEdit {
    pub commands: Vec<Command>,
    /// The bitmap the element shows once they are applied.
    pub bitmap: BitmapId,
    /// The element.
    pub at: SpanRef,
}

/// A Blur, Smudge or Raster Brush stroke in progress on one bitmap element.
pub struct BlendStroke {
    kind: BlendKind,
    settings: BlendSettings,
    at: SpanRef,
    /// The bitmap the element shows now; `None` while a Raster Brush stroke's new bitmap has not
    /// been placed yet.
    bitmap: Option<BitmapId>,
    name: String,
    /// From the stage to the bitmap's pixels.
    to_pixels: DAffine2,
    /// The full dab width in pixels.
    diameter: f32,
    canvas: Canvas,
    /// The last point reached, in pixels, with its pressure.
    last: Option<(DVec2, f32)>,
    /// How far the stroke has gone since the last dab, and how far apart the next one is.
    since: f64,
    next: f64,
    /// Smudge: the color carried along, premultiplied.
    carry: Option<Premul>,
    /// Raster Brush: its paint and the color it has picked up.
    painting: Option<Painting>,
    /// Raster Brush on a new bitmap: where the bitmap goes when its first edit is made.
    unplaced: Option<Unplaced>,
    dabs: usize,
}

impl BlendStroke {
    /// Starts a Blur or Smudge stroke at `start` on the topmost bitmap under it at `frame` (on a
    /// layer that is shown and unlocked, as the Magic Wand finds it). Lays no dab yet. The Raster
    /// Brush starts with [`BlendStroke::paint`], since it needs a color and a layer.
    pub fn begin(
        document: &Document,
        scene: SceneId,
        frame: u32,
        start: Pos2,
        kind: BlendKind,
        settings: BlendSettings,
    ) -> Result<Self, BlendError> {
        if kind == BlendKind::RasterBrush {
            return Err(BlendError::NeedsPaint);
        }
        settings.validate()?;
        let hit = bitmap_at(document, scene, frame, start).ok_or(BlendError::NoBitmap)?;
        Self::on_bitmap(document, scene, &hit, kind, settings)
    }

    /// A stroke on the bitmap element `hit` names.
    fn on_bitmap(
        document: &Document,
        scene: SceneId,
        hit: &BitmapHit,
        kind: BlendKind,
        settings: BlendSettings,
    ) -> Result<Self, BlendError> {
        let Some(Content::Bitmap(placed)) = find_element(document, scene, hit.at) else {
            return Err(BlendError::NotBitmap);
        };
        let bitmap = document.bitmap(hit.bitmap).ok_or(BlendError::NotBitmap)?;
        let matrix = placed.transform().as_affine();
        let area = matrix.matrix2.determinant().abs();
        if !area.is_finite() || area < 1e-12 {
            return Err(BlendError::Flat);
        }
        let per_unit = (1.0 / area.sqrt()) as f32;
        let canvas = Canvas::new(
            bitmap.width(),
            bitmap.height(),
            bitmap.shared_pixels(),
            blur_radius(kind, settings.size * per_unit),
        );
        Ok(Self::new(
            kind,
            settings,
            hit.at,
            Some(hit.bitmap),
            bitmap.name().to_owned(),
            matrix.inverse(),
            per_unit,
            canvas,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn new(
        kind: BlendKind,
        settings: BlendSettings,
        at: SpanRef,
        bitmap: Option<BitmapId>,
        name: String,
        to_pixels: DAffine2,
        per_unit: f32,
        canvas: Canvas,
    ) -> Self {
        Self {
            kind,
            settings,
            at,
            bitmap,
            name,
            to_pixels,
            diameter: (settings.size * per_unit).min(MAX_DAB_PIXELS),
            canvas,
            last: None,
            since: 0.0,
            next: 0.0,
            carry: None,
            painting: None,
            unplaced: None,
            dabs: 0,
        }
    }

    /// The element the stroke works on.
    pub fn at(&self) -> SpanRef {
        self.at
    }

    pub fn kind(&self) -> BlendKind {
        self.kind
    }

    /// Carries the stroke on through `samples`, in stage coordinates.
    pub fn extend(&mut self, samples: &[InputSample]) {
        for sample in samples {
            let point = self
                .to_pixels
                .transform_point2(DVec2::new(sample.x, sample.y));
            if !point.is_finite() {
                continue;
            }
            let pressure = sample.pressure.clamp(0.0, 1.0);
            match self.last {
                None => {
                    self.dab(point, pressure);
                    self.next = self.interval(pressure);
                }
                Some((from, from_pressure)) => {
                    self.segment(from, from_pressure, point, pressure);
                }
            }
            self.last = Some((point, pressure));
        }
    }

    /// Lays dabs from `a` to `b`, each `next` on from the last, carrying what is left over on to
    /// the next segment so the spacing does not depend on how the drag was sampled. Only the part
    /// of the segment near the bitmap is walked, so a point far off it costs nothing.
    fn segment(&mut self, a: DVec2, pa: f32, b: DVec2, pb: f32) {
        let length = a.distance(b);
        if length <= 0.0 {
            return;
        }
        let reach = f64::from(self.diameter) + 2.0;
        let (w, h) = (f64::from(self.canvas.width), f64::from(self.canvas.height));
        let Some((enter, leave)) = clip(a, b, [-reach, -reach, w + reach, h + reach]) else {
            self.since += length;
            return;
        };
        self.since += enter * length;
        let mut along = (self.next - self.since).max(0.0) + enter * length;
        let end = leave * length;
        let mut last_dab = None;
        while along <= end && self.dabs < MAX_DABS {
            let t = along / length;
            let pressure = pa + (pb - pa) * t as f32;
            self.dab(a.lerp(b, t), pressure);
            last_dab = Some(along);
            self.next = self.interval(pressure);
            along += self.next;
        }
        self.since = match last_dab {
            Some(at) => length - at,
            None => self.since + (length - enter * length),
        };
    }

    /// The dab width at `pressure`, in pixels.
    fn size_at(&self, pressure: f32) -> f32 {
        if self.settings.pressure_size {
            self.diameter * pressure
        } else {
            self.diameter
        }
    }

    /// How far on the next dab is: a fraction of the dab's size, at least a pixel.
    fn interval(&self, pressure: f32) -> f64 {
        let size = self.size_at(pressure);
        f64::from((dab::spacing(self.settings.hardness) * size).max(1.0))
    }

    fn dab(&mut self, at: DVec2, pressure: f32) {
        if self.dabs >= MAX_DABS {
            return;
        }
        self.dabs += 1;
        let size = self.size_at(pressure);
        // Blur and Smudge build up dab by dab, closer dabs each adding less; the Raster Brush
        // takes the strongest dab at each pixel, so each lays at full strength.
        let strength = if self.kind == BlendKind::RasterBrush {
            1.0
        } else {
            dab::per_dab(self.interval(pressure) as f32 / size.max(f32::EPSILON))
        };
        let dab = Dab::new(
            at.x as f32,
            at.y as f32,
            size,
            self.settings.hardness,
            strength,
        );
        let ceiling = self.settings.strength;
        // How quickly the carried color follows what is under the brush: slowly as the stretch
        // is long, quickly as it is short.
        let keep = 5.0 / (f32::from(self.settings.stretch) + 5.0);
        match self.kind {
            BlendKind::Blur => self.canvas.blur_dab(&dab, ceiling),
            BlendKind::Smudge => {
                let under = self.canvas.smudge_dab(&dab, ceiling, self.carry);
                if let Some(under) = under {
                    self.carry = Some(match self.carry {
                        Some(carried) => canvas::lerp(carried, under, keep),
                        None => under,
                    });
                }
            }
            BlendKind::RasterBrush => {
                let Some(painting) = self.painting.as_mut() else {
                    return;
                };
                let mix = self.settings.mix;
                // With no mix the color under the brush is never read, so a plain brush costs
                // only the laying.
                let under = if mix > 0.0 {
                    self.canvas.sample(&dab)
                } else {
                    None
                };
                let color = painting.next_color(under, mix, keep);
                self.canvas.lay_dab(&dab, ceiling, color);
            }
        }
    }

    /// Whether the stroke has changed a pixel since its last edit was applied.
    pub fn changed(&self) -> bool {
        self.canvas.dirty
    }

    /// The commands that show the stroke's pixels on its element: a new bitmap in the library
    /// (or one already there with the same pixels), the element showing it, and the bitmap it
    /// showed taken out of the library when nothing else shows it. One batch, so one undo step.
    pub fn edit(&self, document: &Document, scene: SceneId) -> Result<BlendEdit, BlendError> {
        if let Some(unplaced) = &self.unplaced {
            return self.place(document, scene, unplaced);
        }
        let Some(Content::Bitmap(placed)) = find_element(document, scene, self.at) else {
            return Err(BlendError::NotBitmap);
        };
        let old = document
            .bitmap(placed.bitmap())
            .ok_or(BlendError::NotBitmap)?;
        if Some(placed.bitmap()) != self.bitmap
            || old.width() != self.canvas.width
            || old.height() != self.canvas.height
        {
            return Err(BlendError::Stale);
        }
        let new = self.made()?;
        if new.content_hash() == old.content_hash() {
            return Err(BlendError::NothingChanged);
        }
        let mut commands = Vec::new();
        let id = in_library(document, new, &mut commands);
        let content = Content::Bitmap(
            BitmapInstance::new(placed.id(), id, placed.transform()).with_opacity(placed.opacity()),
        );
        commands.extend(
            replace_element(document, scene, self.at, content).ok_or(BlendError::NotBitmap)?,
        );
        if uses(document, placed.bitmap()) == 1 {
            commands.push(Command::RemoveBitmap(placed.bitmap()));
        }
        Ok(BlendEdit {
            commands,
            bitmap: id,
            at: self.at,
        })
    }

    /// The bitmap of the stroke's pixels as they are now.
    fn made(&self) -> Result<Bitmap, BlendError> {
        Bitmap::new(
            BitmapId::new(),
            self.name.clone(),
            self.canvas.width,
            self.canvas.height,
            self.canvas.working.clone(),
        )
        .map_err(|e| BlendError::Bitmap(e.to_string()))
    }

    /// The bitmap the element shows, which the next edit is refused as stale without; `None`
    /// before a Raster Brush stroke's new bitmap is placed.
    pub fn bitmap(&self) -> Option<BitmapId> {
        self.bitmap
    }

    /// Records that `edit` was applied, with every change the stroke has made so far, so the next
    /// edit builds on it. A new bitmap's first edit places it: the stroke goes on on that element.
    pub fn shows(&mut self, edit: &BlendEdit) {
        self.bitmap = Some(edit.bitmap);
        self.at = edit.at;
        self.unplaced = None;
        self.canvas.dirty = false;
    }

    /// Records that an edit was found to change nothing, so the stroke waits for its next change.
    pub fn unchanged(&mut self) {
        self.canvas.dirty = false;
    }
}

/// `bitmap`'s id in the library: one already there with the same pixels, else its own, with the
/// command that adds it.
pub(crate) fn in_library(
    document: &Document,
    bitmap: Bitmap,
    commands: &mut Vec<Command>,
) -> BitmapId {
    match document
        .bitmaps()
        .iter()
        .find(|b| b.content_hash() == bitmap.content_hash())
    {
        Some(same) => same.id(),
        None => {
            let id = bitmap.id();
            commands.push(Command::AddBitmap(bitmap));
            id
        }
    }
}

/// The blur radius of a Blur stroke whose dabs are `diameter` pixels wide; `None` for the others.
fn blur_radius(kind: BlendKind, diameter: f32) -> Option<u32> {
    // The blur reaches about a sixth of the dab, as a blur linked to the brush size does.
    (kind == BlendKind::Blur)
        .then(|| ((diameter.min(MAX_DAB_PIXELS) / 12.0).round() as u32).clamp(1, 24))
}

/// A whole stroke through `points` at once, as a front end with no pointer gives it: the
/// commands that blur or smudge the bitmap under the first point.
pub fn rub(
    document: &Document,
    scene: SceneId,
    frame: u32,
    kind: BlendKind,
    settings: BlendSettings,
    points: &[InputSample],
) -> Result<BlendEdit, BlendError> {
    let first = points.first().ok_or(BlendError::NoPoints)?;
    if points.len() > MAX_POINTS {
        return Err(BlendError::TooManyPoints);
    }
    let start = Pos2::new(first.x as f32, first.y as f32);
    let mut stroke = BlendStroke::begin(document, scene, frame, start, kind, settings)?;
    stroke.extend(points);
    if !stroke.changed() {
        return Err(BlendError::NothingChanged);
    }
    stroke.edit(document, scene)
}

/// The part of the segment from `a` to `b` inside `[x0, y0, x1, y1]`, as the fractions of the
/// way along where it goes in and comes out.
fn clip(a: DVec2, b: DVec2, [x0, y0, x1, y1]: [f64; 4]) -> Option<(f64, f64)> {
    let d = b - a;
    let (mut enter, mut leave) = (0.0f64, 1.0f64);
    for (p, q) in [
        (-d.x, a.x - x0),
        (d.x, x1 - a.x),
        (-d.y, a.y - y0),
        (d.y, y1 - a.y),
    ] {
        if p == 0.0 {
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let t = q / p;
        if p < 0.0 {
            enter = enter.max(t);
        } else {
            leave = leave.min(t);
        }
    }
    (enter <= leave).then_some((enter, leave))
}

#[cfg(test)]
mod tests;
