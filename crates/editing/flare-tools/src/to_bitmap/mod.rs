//! Convert to Bitmap, as Animate's Modify menu has it: the selected shapes, instances and bitmaps
//! of one keyframe become one bitmap of how they look, in the same place on the same layer, so the
//! Raster Brush, Blur and Smudge can work on them. It goes one way: the pixels keep no outlines.
//!
//! This crate has no renderer, so the work is in two halves: [`plan`] checks the selection and
//! says what to draw and at what size, a renderer draws it, and [`place`] turns those pixels into
//! the edit, one undo step. The bitmap takes the place of the topmost selected element.

use flare_commands::Command;
use flare_core::{BitmapId, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, MAX_BITMAP_PIXELS, MAX_BITMAP_SIDE, Tween,
};
use glamx::{DAffine2, DVec2};
use thiserror::Error;

use crate::blend::in_library;
use crate::common::{SpanRef, content_bounds, find_span, replace_span, scene_timeline};
use crate::magic_wand::uses;

/// The most bitmap pixels to a stage unit, along each side.
pub const MAX_SCALE: u32 = 4;
/// How far past the outlines the picture reaches, in stage units, so anti-aliased edges and
/// pointed corners of a line are not cut; the clear margin is trimmed off afterwards.
const MARGIN: f64 = 2.0;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ToBitmapError {
    #[error("select the shapes to convert to a bitmap")]
    NothingSelected,
    #[error(
        "the selection is on more than one keyframe or layer; convert one keyframe's at a time"
    )]
    NotOneKeyframe,
    #[error("the selection is not in the document any more")]
    Missing,
    #[error("there is no such layer in the scene")]
    NoSuchLayer,
    #[error("the layer has no keyframe at that frame")]
    NoKeyframe,
    #[error("the keyframe is tweened, so what it shows changes from frame to frame")]
    Tweened,
    #[error("the layer is locked")]
    Locked,
    #[error("only shapes, symbol instances and bitmaps can be converted to a bitmap")]
    NotDrawable,
    #[error("the selection draws nothing")]
    Empty,
    #[error("the bitmap would be {0}x{1} pixels, larger than a bitmap may be")]
    TooLarge(u64, u64),
    #[error("the scale is 1 to {MAX_SCALE} pixels to a stage unit")]
    Scale,
    #[error("the pixels do not fit the plan")]
    Pixels,
    #[error("the bitmap could not be made: {0}")]
    Bitmap(String),
}

/// What to draw for a conversion: `contents`, bottom first, over the stage area at `x`, `y`,
/// `scale` pixels to a unit, into a `width` by `height` picture with a clear background.
#[derive(Debug, Clone, PartialEq)]
pub struct ToBitmapPlan {
    /// The keyframe, and in it the topmost selected element, where the bitmap goes.
    pub at: SpanRef,
    /// The selected elements' places in the keyframe, bottom first.
    pub elements: Vec<usize>,
    pub contents: Vec<Content>,
    pub x: f64,
    pub y: f64,
    pub scale: u32,
    pub width: u32,
    pub height: u32,
}

/// The edit that converts: apply `commands`, after which `at` is the bitmap element.
#[derive(Debug, Clone, PartialEq)]
pub struct ToBitmapEdit {
    pub commands: Vec<Command>,
    pub at: SpanRef,
    pub bitmap: BitmapId,
}

/// Checks that `selection` can be converted at `scale` pixels to a stage unit and says what to
/// draw. Everything must be on one keyframe that is not tweened, on a layer that is not locked.
pub fn plan(
    document: &Document,
    scene: SceneId,
    selection: &[SpanRef],
    scale: u32,
) -> Result<ToBitmapPlan, ToBitmapError> {
    if !(1..=MAX_SCALE).contains(&scale) {
        return Err(ToBitmapError::Scale);
    }
    let first = *selection.first().ok_or(ToBitmapError::NothingSelected)?;
    if selection
        .iter()
        .any(|s| s.layer != first.layer || s.start != first.start)
    {
        return Err(ToBitmapError::NotOneKeyframe);
    }
    let layer = scene_timeline(document, scene)
        .and_then(|t| t.layers().iter().find(|l| l.id() == first.layer))
        .ok_or(ToBitmapError::Missing)?;
    if layer.is_locked() {
        return Err(ToBitmapError::Locked);
    }
    let span = find_span(document, scene, first).ok_or(ToBitmapError::Missing)?;
    if span.tween() != Tween::None {
        return Err(ToBitmapError::Tweened);
    }
    let mut elements: Vec<usize> = selection.iter().map(|s| s.element).collect();
    elements.sort_unstable();
    elements.dedup();
    let mut contents = Vec::with_capacity(elements.len());
    let mut area: Option<[f64; 4]> = None;
    let mut widest = 0.0f64;
    for &index in &elements {
        let content = span.element(index).ok_or(ToBitmapError::Missing)?;
        if !matches!(
            content,
            Content::Shape(_) | Content::Instance(_) | Content::Bitmap(_)
        ) {
            return Err(ToBitmapError::NotDrawable);
        }
        if let Content::Shape(shape) = content {
            widest = widest.max(shape.stroke().map_or(0.0, |s| s.width()));
        }
        if let Some(b) = content_bounds(document, content) {
            let b = [
                f64::from(b.min.x),
                f64::from(b.min.y),
                f64::from(b.max.x),
                f64::from(b.max.y),
            ];
            area = Some(match area {
                Some(a) => [
                    a[0].min(b[0]),
                    a[1].min(b[1]),
                    a[2].max(b[2]),
                    a[3].max(b[3]),
                ],
                None => b,
            });
        }
        contents.push(content.clone());
    }
    let [x0, y0, x1, y1] = area.ok_or(ToBitmapError::Empty)?;
    // Past the half width the bounds count, a pointed corner of a thick line reaches further.
    let margin = MARGIN + widest / 2.0;
    let (x, y) = ((x0 - margin).floor(), (y0 - margin).floor());
    let side = |from: f64, to: f64| ((to + margin).ceil() - from).max(1.0) * f64::from(scale);
    let (w, h) = (side(x, x1), side(y, y1));
    if !(w.is_finite() && h.is_finite()) {
        return Err(ToBitmapError::Empty);
    }
    let (w, h) = (w as u64, h as u64);
    if w > u64::from(MAX_BITMAP_SIDE) || h > u64::from(MAX_BITMAP_SIDE) || w * h > MAX_BITMAP_PIXELS
    {
        return Err(ToBitmapError::TooLarge(w, h));
    }
    let top = elements.last().copied().unwrap_or(first.element);
    Ok(ToBitmapPlan {
        at: first.with_element(top),
        elements,
        contents,
        x,
        y,
        scale,
        width: w as u32,
        height: h as u32,
    })
}

/// What a front end with no selection names: the elements of `layer`'s keyframe at `frame`
/// (by default the topmost layer), all of them or those at `elements` (counting from the bottom,
/// as `flare info` lists them).
pub fn keyframe_selection(
    document: &Document,
    scene: SceneId,
    layer: Option<LayerId>,
    frame: u32,
    elements: Option<&[usize]>,
) -> Result<Vec<SpanRef>, ToBitmapError> {
    let layers = scene_timeline(document, scene)
        .ok_or(ToBitmapError::Missing)?
        .layers();
    let layer = match layer {
        Some(id) => layers.iter().find(|l| l.id() == id),
        None => layers.first(),
    }
    .ok_or(ToBitmapError::NoSuchLayer)?;
    let span = layer.span_at(frame).ok_or(ToBitmapError::NoKeyframe)?;
    let at = SpanRef {
        layer: layer.id(),
        start: span.start(),
        element: 0,
    };
    let all: Vec<usize> = (0..span.element_count()).collect();
    let chosen = elements.unwrap_or(&all);
    if chosen.iter().any(|&i| i >= span.element_count()) {
        return Err(ToBitmapError::Missing);
    }
    Ok(chosen.iter().map(|&i| at.with_element(i)).collect())
}

/// The edit that puts `rgba` (straight alpha, the plan's width by height, as the plan's contents
/// were drawn) in their place: a bitmap named `name` trimmed to what is not clear, placed where it
/// was drawn, taking the topmost selected element's place. The selected bitmaps no longer shown
/// anywhere leave the library. One batch, so one undo step.
pub fn place(
    document: &Document,
    scene: SceneId,
    plan: &ToBitmapPlan,
    rgba: &[u8],
    name: &str,
) -> Result<ToBitmapEdit, ToBitmapError> {
    let (w, h) = (plan.width as usize, plan.height as usize);
    if rgba.len() != w * h * 4 {
        return Err(ToBitmapError::Pixels);
    }
    let [left, top, right, bottom] = opaque_bounds(rgba, w, h).ok_or(ToBitmapError::Empty)?;
    let mut pixels = Vec::with_capacity((right - left) * (bottom - top) * 4);
    for row in top..bottom {
        pixels.extend_from_slice(&rgba[(row * w + left) * 4..(row * w + right) * 4]);
    }
    let bitmap = Bitmap::new(
        BitmapId::new(),
        name,
        (right - left) as u32,
        (bottom - top) as u32,
        pixels,
    )
    .map_err(|e| ToBitmapError::Bitmap(e.to_string()))?;
    let span = find_span(document, scene, plan.at).ok_or(ToBitmapError::Missing)?;
    let mut commands = Vec::new();
    let id = in_library(document, bitmap, &mut commands);
    let per_pixel = 1.0 / f64::from(plan.scale);
    let matrix = DAffine2::from_scale_angle_translation(
        DVec2::splat(per_pixel),
        0.0,
        DVec2::new(
            plan.x + left as f64 * per_pixel,
            plan.y + top as f64 * per_pixel,
        ),
    );
    let transform = Transform2D::new(matrix).map_err(|e| ToBitmapError::Bitmap(e.to_string()))?;
    let placed = Content::Bitmap(BitmapInstance::new(InstanceId::new(), id, transform));
    let top_element = plan
        .elements
        .last()
        .copied()
        .ok_or(ToBitmapError::NothingSelected)?;
    let mut kept = Vec::with_capacity(span.element_count());
    let mut at = 0;
    for (index, content) in span.elements().enumerate() {
        if index == top_element {
            at = kept.len();
            kept.push(placed.clone());
        } else if plan.elements.binary_search(&index).is_err() {
            kept.push(content.clone());
        }
    }
    let new_span = span
        .with_elements(kept)
        .map_err(|e| ToBitmapError::Bitmap(e.to_string()))?
        .ok_or(ToBitmapError::Missing)?;
    commands.extend(replace_span(scene, plan.at, new_span));
    let mut dropped: Vec<BitmapId> = Vec::new();
    for &index in &plan.elements {
        if let Some(Content::Bitmap(old)) = span.element(index)
            && old.bitmap() != id
            && !dropped.contains(&old.bitmap())
        {
            let shown_here = plan
                .elements
                .iter()
                .filter(|&&i| matches!(span.element(i), Some(Content::Bitmap(b)) if b.bitmap() == old.bitmap()))
                .count();
            if uses(document, old.bitmap()) == shown_here {
                dropped.push(old.bitmap());
            }
        }
    }
    commands.extend(dropped.into_iter().map(Command::RemoveBitmap));
    Ok(ToBitmapEdit {
        commands,
        at: plan.at.with_element(at),
        bitmap: id,
    })
}

/// The smallest box, `[left, top, right, bottom)` in pixels, holding every pixel that is not
/// fully clear; `None` when all are.
fn opaque_bounds(rgba: &[u8], w: usize, h: usize) -> Option<[usize; 4]> {
    let mut found: Option<[usize; 4]> = None;
    for row in 0..h {
        let (line, _) = rgba[row * w * 4..(row + 1) * w * 4].as_chunks::<4>();
        let Some(first) = line.iter().position(|p| p[3] != 0) else {
            continue;
        };
        let last = line.iter().rposition(|p| p[3] != 0).unwrap_or(first);
        found = Some(match found {
            Some([l, t, r, _]) => [l.min(first), t, r.max(last + 1), row + 1],
            None => [first, row, last + 1, row + 1],
        });
    }
    found
}

#[cfg(test)]
mod tests;
