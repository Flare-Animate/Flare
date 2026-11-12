//! The Magic Wand: click a bitmap on the stage to pick the pixels of a color like the one
//! clicked, add to or take from what is picked, and clear the picked pixels (or all the others),
//! which is how a background is taken off a picture. Animate's wand works on a bitmap that was
//! broken apart into a fill; Flare's works on the bitmap itself, and a clear makes a new bitmap
//! for that one element, so other frames that show the same picture keep it whole.

mod mask;

pub use mask::{Mask, coverage, pick};

/// Points on the stage, as the wand takes them; named here so a caller needs no `emath` of its own.
pub use emath::Pos2;
use flare_commands::Command;
use flare_core::{BitmapId, SceneId};
use flare_document::{Bitmap, BitmapInstance, Content, Document, Layer};
use glamx::DVec2;
use thiserror::Error;

use crate::common::{SpanRef, find_element, replace_element, scene_timeline};

/// The highest threshold, as in Animate.
pub const MAX_THRESHOLD: u8 = 200;
/// Animate's starting threshold.
pub const DEFAULT_THRESHOLD: u8 = 20;
/// Most points one `pick_points` takes.
pub const MAX_POINTS: usize = 64;

/// How soft the edge a clear leaves is, as Animate's four Smoothing settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Smoothing {
    /// The edge follows the pixels exactly.
    Pixels,
    Rough,
    #[default]
    Normal,
    Smooth,
}

impl Smoothing {
    pub const ALL: [Smoothing; 4] = [Self::Pixels, Self::Rough, Self::Normal, Self::Smooth];

    /// Over how many pixels the edge fades.
    pub fn feather(self) -> f32 {
        match self {
            Self::Pixels => 0.0,
            Self::Rough => 1.0,
            Self::Normal => 2.0,
            Self::Smooth => 3.0,
        }
    }

    /// The word for it on a command line and in a message key.
    pub fn name(self) -> &'static str {
        match self {
            Self::Pixels => "pixels",
            Self::Rough => "rough",
            Self::Normal => "normal",
            Self::Smooth => "smooth",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|s| s.name().eq_ignore_ascii_case(word.trim()))
    }
}

/// The wand's settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WandSettings {
    /// How far a color may be from the clicked one and still be picked, 0 to `MAX_THRESHOLD`.
    pub threshold: u8,
    /// Only pixels joined to the clicked one; off, every like pixel in the picture.
    pub contiguous: bool,
    pub smoothing: Smoothing,
}

impl Default for WandSettings {
    fn default() -> Self {
        Self {
            threshold: DEFAULT_THRESHOLD,
            contiguous: true,
            smoothing: Smoothing::Normal,
        }
    }
}

/// What a click does to what is already picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combine {
    /// Starts again from what this click picks.
    Replace,
    /// Shift: adds it.
    Add,
    /// Alt: takes it away.
    Subtract,
}

/// The pixels picked in one bitmap element.
#[derive(Debug, Clone, PartialEq)]
pub struct WandSelection {
    pub at: SpanRef,
    /// The picture the mask was made on; a selection of another picture is stale.
    pub bitmap: BitmapId,
    pub mask: Mask,
}

/// The bitmap element under a point, and the pixel the point is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitmapHit {
    pub at: SpanRef,
    pub bitmap: BitmapId,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WandError {
    #[error("the Magic Wand works on bitmaps; there is no bitmap at that point")]
    NoBitmap,
    #[error("the element is not a bitmap any more")]
    NotBitmap,
    #[error("the picture changed since its pixels were picked")]
    Stale,
    #[error("no pixels are picked")]
    NothingPicked,
    #[error("the picked pixels are already clear")]
    NothingToClear,
    #[error("the points are on different bitmaps; pick from one bitmap at a time")]
    DifferentBitmaps,
    #[error("give at least one point")]
    NoPoints,
    #[error("at most {MAX_POINTS} points at a time")]
    TooManyPoints,
    #[error("the threshold is 0 to {MAX_THRESHOLD}")]
    Threshold,
    #[error("the bitmap could not be rebuilt: {0}")]
    Bitmap(String),
}

/// The topmost bitmap under `point` (in the coordinates of the timeline being edited) at
/// `frame`, on a layer that is shown and not locked. A bitmap inside a group or a symbol is not
/// reached; edit that in place.
pub fn bitmap_at(
    document: &Document,
    scene: SceneId,
    frame: u32,
    point: Pos2,
) -> Option<BitmapHit> {
    let layers = scene_timeline(document, scene)?.layers();
    let point = DVec2::new(f64::from(point.x), f64::from(point.y));
    for (index, layer) in layers.iter().enumerate() {
        if layer.is_folder() || layer.is_camera() || layer.is_guide() || !open(layers, index) {
            continue;
        }
        let Some(span) = layer.span_at(frame) else {
            continue;
        };
        let elements: Vec<&Content> = span.elements().collect();
        for (element, content) in elements.iter().enumerate().rev() {
            let Content::Bitmap(placed) = content else {
                continue;
            };
            let Some(bitmap) = document.bitmap(placed.bitmap()) else {
                continue;
            };
            let Some((x, y)) = pixel_under(placed, bitmap, point) else {
                continue;
            };
            return Some(BitmapHit {
                at: SpanRef {
                    layer: layer.id(),
                    start: span.start(),
                    element,
                },
                bitmap: placed.bitmap(),
                x,
                y,
            });
        }
    }
    None
}

fn pixel_under(placed: &BitmapInstance, bitmap: &Bitmap, point: DVec2) -> Option<(u32, u32)> {
    let matrix = placed.transform().as_affine();
    if matrix.matrix2.determinant().abs() < 1e-12 {
        return None;
    }
    let local = matrix.inverse().transform_point2(point);
    let (x, y) = (local.x.floor(), local.y.floor());
    let inside =
        x >= 0.0 && y >= 0.0 && x < f64::from(bitmap.width()) && y < f64::from(bitmap.height());
    inside.then_some((x as u32, y as u32))
}

/// Whether `layers[index]` can be picked from: shown and unlocked, and so is every folder it is in.
fn open(layers: &[Layer], index: usize) -> bool {
    let mut layer = &layers[index];
    loop {
        if !layer.is_visible() || layer.is_locked() {
            return false;
        }
        let Some(parent) = layer.parent() else {
            return true;
        };
        match layers[..index].iter().find(|l| l.id() == parent) {
            Some(folder) => layer = folder,
            None => return true,
        }
    }
}

/// A click of the wand at `point`: what it picks, combined with `current` as `combine` says.
/// A click on another bitmap than the current selection's starts again there; one that takes
/// everything away leaves `None`.
pub fn click(
    document: &Document,
    scene: SceneId,
    frame: u32,
    point: Pos2,
    settings: &WandSettings,
    combine: Combine,
    current: Option<WandSelection>,
) -> Result<Option<WandSelection>, WandError> {
    let hit = bitmap_at(document, scene, frame, point).ok_or(WandError::NoBitmap)?;
    let bitmap = document.bitmap(hit.bitmap).ok_or(WandError::NotBitmap)?;
    let threshold = settings.threshold.min(MAX_THRESHOLD);
    let picked =
        pick(bitmap, hit.x, hit.y, threshold, settings.contiguous).ok_or(WandError::NoBitmap)?;
    let same = current.filter(|c| c.at == hit.at && c.bitmap == hit.bitmap);
    let mask = match (combine, same) {
        (Combine::Add, Some(mut c)) => {
            c.mask.add(&picked);
            c.mask
        }
        (Combine::Subtract, Some(mut c)) => {
            c.mask.subtract(&picked);
            c.mask
        }
        // Taking away from nothing leaves nothing.
        (Combine::Subtract, None) => return Ok(None),
        _ => picked,
    };
    Ok((!mask.is_empty()).then_some(WandSelection {
        at: hit.at,
        bitmap: hit.bitmap,
        mask,
    }))
}

/// What clicks at each of `points` pick together, as a front end with no pointer asks: the first
/// picks, the others add, and all must land on the same bitmap. `invert` then picks every other
/// pixel instead (to keep only what the points picked).
pub fn pick_points(
    document: &Document,
    scene: SceneId,
    frame: u32,
    points: &[Pos2],
    settings: &WandSettings,
    invert: bool,
) -> Result<WandSelection, WandError> {
    if settings.threshold > MAX_THRESHOLD {
        return Err(WandError::Threshold);
    }
    if points.len() > MAX_POINTS {
        return Err(WandError::TooManyPoints);
    }
    let mut picked: Option<WandSelection> = None;
    for point in points {
        let first = picked.as_ref().map(|p| p.at);
        let combine = if first.is_some() {
            Combine::Add
        } else {
            Combine::Replace
        };
        let next = click(
            document,
            scene,
            frame,
            *point,
            settings,
            combine,
            picked.take(),
        )?;
        if let (Some(first), Some(next)) = (first, &next)
            && next.at != first
        {
            return Err(WandError::DifferentBitmaps);
        }
        picked = next;
    }
    let mut selection = picked.ok_or(WandError::NoPoints)?;
    if invert {
        selection.mask.invert();
    }
    Ok(selection)
}

/// The commands that clear the picked pixels of `selection`, with an edge as soft as
/// `smoothing`: a new bitmap in the library for that element (or one already there with the same
/// pixels), and the old one taken out of the library when nothing else shows it. One batch, so
/// one undo step.
pub fn clear(
    document: &Document,
    scene: SceneId,
    selection: &WandSelection,
    smoothing: Smoothing,
) -> Result<Vec<Command>, WandError> {
    let Some(Content::Bitmap(placed)) = find_element(document, scene, selection.at) else {
        return Err(WandError::NotBitmap);
    };
    let old = document
        .bitmap(placed.bitmap())
        .ok_or(WandError::NotBitmap)?;
    let mask = &selection.mask;
    if placed.bitmap() != selection.bitmap
        || mask.width() != old.width()
        || mask.height() != old.height()
    {
        return Err(WandError::Stale);
    }
    if mask.is_empty() {
        return Err(WandError::NothingPicked);
    }
    let keep = coverage(mask, smoothing.feather());
    let mut rgba = old.pixels().to_vec();
    for (pixel, kept) in rgba.as_chunks_mut::<4>().0.iter_mut().zip(keep) {
        pixel[3] = (f32::from(pixel[3]) * kept).round() as u8;
        if pixel[3] == 0 {
            pixel[..3].fill(0);
        }
    }
    if rgba == old.pixels() {
        return Err(WandError::NothingToClear);
    }
    let new = Bitmap::new(BitmapId::new(), old.name(), old.width(), old.height(), rgba)
        .map_err(|e| WandError::Bitmap(e.to_string()))?;
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
        replace_element(document, scene, selection.at, content).ok_or(WandError::NotBitmap)?,
    );
    if uses(document, placed.bitmap()) == 1 {
        commands.push(Command::RemoveBitmap(placed.bitmap()));
    }
    Ok(commands)
}

/// How many elements, in every scene and symbol, show bitmap `id`.
fn uses(document: &Document, id: BitmapId) -> usize {
    document
        .scenes()
        .iter()
        .map(|s| s.timeline())
        .chain(document.symbols().iter().map(|s| s.timeline()))
        .flat_map(|t| t.layers())
        .flat_map(|l| l.spans())
        .flat_map(|s| s.elements())
        .filter(|c| matches!(c, Content::Bitmap(b) if b.bitmap() == id))
        .count()
}

#[cfg(test)]
mod tests;
