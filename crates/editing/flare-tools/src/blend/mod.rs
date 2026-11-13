//! Blur and Smudge: drag over a bitmap to soften it, or to push its colors along, as Clip Studio
//! Paint's blending tools do. Both work on the bitmap under the start of the stroke, in its own
//! pixels, and a stroke makes a new bitmap for that one element (as a Magic Wand clear does), so
//! other frames that show the same picture keep it as it was.
//!
//! A stroke is a row of round dabs along the drag, spaced by a fraction of their size. Blur moves
//! each pixel a dab covers towards a blurred copy of the bitmap; Smudge picks up the colors under
//! each dab, carries a running mix of them along, and lays it down at the next. The stroke is
//! capped at its strength (see `canvas`), and pen pressure sets the size of each dab.

mod canvas;
mod dab;

use flare_commands::Command;
use flare_core::{BitmapId, SceneId};
use flare_document::{Bitmap, BitmapInstance, Content, Document};
/// The samples a stroke takes; named here so a caller needs no `flare-input` of its own.
pub use flare_input::{InputSample, PointerKind};
use glamx::{DAffine2, DVec2};
use thiserror::Error;

use crate::common::{SpanRef, find_element, replace_element};
use crate::magic_wand::{Pos2, bitmap_at, uses};
use canvas::{Canvas, Premul};
use dab::Dab;

/// The largest size, in stage units.
pub const MAX_SIZE: f32 = 1000.0;
/// The widest a dab is drawn, in bitmap pixels, however the bitmap is scaled.
pub const MAX_DAB_PIXELS: f32 = 1024.0;
/// The most Color stretch.
pub const MAX_STRETCH: u8 = 100;
/// Most points one [`rub`] takes.
pub const MAX_POINTS: usize = 4096;
/// Most dabs one stroke lays; past this the stroke stops changing the bitmap.
pub const MAX_DABS: usize = 200_000;

/// Which of the two tools a stroke is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendKind {
    /// Softens what it passes over.
    Blur,
    /// Pushes colors along the drag, as a finger through wet paint.
    Smudge,
}

impl BlendKind {
    pub const ALL: [BlendKind; 2] = [Self::Blur, Self::Smudge];

    /// The word for it on a command line and in a message key.
    pub fn name(self) -> &'static str {
        match self {
            Self::Blur => "blur",
            Self::Smudge => "smudge",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|k| k.name().eq_ignore_ascii_case(word.trim()))
    }
}

/// A Blur or Smudge tool's settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BlendSettings {
    /// The width of a dab at full pressure, in stage units, 1 to [`MAX_SIZE`].
    pub size: f32,
    /// How far a stroke goes at most, 0 to 1: at 1 the middle of a Blur stroke is the blurred
    /// picture and the middle of a Smudge stroke is the carried color.
    pub strength: f32,
    /// How sharp a dab's edge is, 0 (soft) to 1.
    pub hardness: f32,
    /// Pen pressure sets the size of each dab; off, every dab is full size.
    pub pressure_size: bool,
    /// Smudge only: how far a picked-up color is carried, 0 (hardly at all, which mixes in
    /// place) to [`MAX_STRETCH`].
    pub stretch: u8,
}

impl BlendSettings {
    /// Where each tool starts, from Clip Studio's own Blur and Finger tip.
    pub fn for_kind(kind: BlendKind) -> Self {
        match kind {
            BlendKind::Blur => Self {
                size: 80.0,
                strength: 1.0,
                hardness: 0.0,
                pressure_size: true,
                stretch: 0,
            },
            BlendKind::Smudge => Self {
                size: 30.0,
                strength: 1.0,
                hardness: 0.5,
                pressure_size: true,
                stretch: 70,
            },
        }
    }

    pub fn validate(&self) -> Result<(), BlendError> {
        if !(1.0..=MAX_SIZE).contains(&self.size) {
            return Err(BlendError::Size);
        }
        if !(0.0..=1.0).contains(&self.strength) {
            return Err(BlendError::Strength);
        }
        if !(0.0..=1.0).contains(&self.hardness) {
            return Err(BlendError::Hardness);
        }
        if self.stretch > MAX_STRETCH {
            return Err(BlendError::Stretch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BlendError {
    #[error("Blur and Smudge work on bitmaps; there is no bitmap where the stroke starts")]
    NoBitmap,
    #[error("the element is not a bitmap any more")]
    NotBitmap,
    #[error("the bitmap changed while the stroke was being made")]
    Stale,
    #[error("the bitmap is squashed flat, so a stroke cannot reach its pixels")]
    Flat,
    #[error("give at least one point")]
    NoPoints,
    #[error("at most {MAX_POINTS} points at a time")]
    TooManyPoints,
    #[error("the size is 1 to {MAX_SIZE}")]
    Size,
    #[error("the strength is 0 to 100%")]
    Strength,
    #[error("the hardness is 0 to 100%")]
    Hardness,
    #[error("the color stretch is 0 to {MAX_STRETCH}")]
    Stretch,
    #[error("the stroke did not change any pixel")]
    NothingChanged,
    #[error("the bitmap could not be rebuilt: {0}")]
    Bitmap(String),
}

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

/// A Blur or Smudge stroke in progress on one bitmap element.
pub struct BlendStroke {
    kind: BlendKind,
    settings: BlendSettings,
    at: SpanRef,
    /// The bitmap the element shows now.
    bitmap: BitmapId,
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
    dabs: usize,
}

impl BlendStroke {
    /// Starts a stroke at `start` on the topmost bitmap under it at `frame` (on a layer that is
    /// shown and unlocked, as the Magic Wand finds it). Lays no dab yet.
    pub fn begin(
        document: &Document,
        scene: SceneId,
        frame: u32,
        start: Pos2,
        kind: BlendKind,
        settings: BlendSettings,
    ) -> Result<Self, BlendError> {
        settings.validate()?;
        let hit = bitmap_at(document, scene, frame, start).ok_or(BlendError::NoBitmap)?;
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
        let diameter = (settings.size * per_unit).min(MAX_DAB_PIXELS);
        // The blur reaches about a sixth of the dab, as a blur linked to the brush size does.
        let blur =
            (kind == BlendKind::Blur).then(|| ((diameter / 12.0).round() as u32).clamp(1, 24));
        Ok(Self {
            kind,
            settings,
            at: hit.at,
            bitmap: hit.bitmap,
            name: bitmap.name().to_owned(),
            to_pixels: matrix.inverse(),
            diameter,
            canvas: Canvas::new(
                bitmap.width(),
                bitmap.height(),
                bitmap.shared_pixels(),
                blur,
            ),
            last: None,
            since: 0.0,
            next: 0.0,
            carry: None,
            dabs: 0,
        })
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
        let ratio = self.interval(pressure) as f32 / size.max(f32::EPSILON);
        let dab = Dab::new(
            at.x as f32,
            at.y as f32,
            size,
            self.settings.hardness,
            dab::per_dab(ratio),
        );
        let ceiling = self.settings.strength;
        match self.kind {
            BlendKind::Blur => self.canvas.blur_dab(&dab, ceiling),
            BlendKind::Smudge => {
                let under = self.canvas.smudge_dab(&dab, ceiling, self.carry);
                if let Some(under) = under {
                    // The carried color follows what is under the brush slowly or quickly as the
                    // stretch is long or short.
                    let keep = 5.0 / (f32::from(self.settings.stretch) + 5.0);
                    self.carry = Some(match self.carry {
                        Some(carried) => canvas::lerp(carried, under, keep),
                        None => under,
                    });
                }
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
        let Some(Content::Bitmap(placed)) = find_element(document, scene, self.at) else {
            return Err(BlendError::NotBitmap);
        };
        let old = document
            .bitmap(placed.bitmap())
            .ok_or(BlendError::NotBitmap)?;
        if placed.bitmap() != self.bitmap
            || old.width() != self.canvas.width
            || old.height() != self.canvas.height
        {
            return Err(BlendError::Stale);
        }
        let new = Bitmap::new(
            BitmapId::new(),
            self.name.clone(),
            self.canvas.width,
            self.canvas.height,
            self.canvas.working.clone(),
        )
        .map_err(|e| BlendError::Bitmap(e.to_string()))?;
        if new.content_hash() == old.content_hash() {
            return Err(BlendError::NothingChanged);
        }
        let mut commands = Vec::new();
        let id = match document
            .bitmaps()
            .iter()
            .find(|b| b.content_hash() == new.content_hash())
        {
            Some(same) => same.id(),
            None => {
                let id = new.id();
                commands.push(Command::AddBitmap(new));
                id
            }
        };
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

    /// The bitmap the stroke takes its element to show: the next edit is refused as stale if
    /// the element shows another.
    pub fn bitmap(&self) -> BitmapId {
        self.bitmap
    }

    /// Records that the element now shows `bitmap`, with every change the stroke has made so far
    /// (after an edit was applied, or found to change nothing), so the next edit builds on it.
    pub fn shows(&mut self, bitmap: BitmapId) {
        self.bitmap = bitmap;
        self.canvas.dirty = false;
    }
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
