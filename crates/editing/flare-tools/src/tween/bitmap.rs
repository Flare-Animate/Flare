//! Bitmap inbetweens: the pictures between two bitmap keyframes, from `flare_tween::raster`.
//! Read only: the document is not changed, and the frames are returned for the caller to write.

use flare_core::SceneId;
use flare_document::{Content, Document};
use flare_tween::raster::{FlowParams, Inbetweener, Raster, RasterError};
use thiserror::Error;

use crate::common::*;

/// The most pictures one request makes. Each is a full bitmap, so the bound keeps a request's
/// output within reason.
pub const MAX_BITMAP_INBETWEENS: u32 = 64;

#[derive(Debug, Error)]
pub enum BitmapInbetweenError {
    #[error("no keyframe starts at frame {0}")]
    NoSuchKeyframe(u32),
    #[error("the keyframe at frame {0} does not hold a bitmap")]
    NotBitmap(u32),
    #[error("bitmap in-betweens need between 1 and {MAX_BITMAP_INBETWEENS}, not {0}")]
    Count(u32),
    #[error(transparent)]
    Raster(#[from] RasterError),
}

/// The in-between pictures, all of one size, in order from the first keyframe to the second.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitmapFrames {
    pub width: u32,
    pub height: u32,
    /// Straight-alpha RGBA, rows top to bottom, one entry per picture.
    pub frames: Vec<Vec<u8>>,
}

/// `count` pictures spaced evenly between the bitmaps of the keyframes `from` and `to`, which are
/// not themselves included. The pictures are the bitmaps' own pixels: an instance's transform and
/// opacity are not applied.
///
/// # Errors
/// See `BitmapInbetweenError`. A flow setting out of range or two bitmaps of different sizes are
/// refused by the raster code.
pub fn bitmap_inbetweens(
    document: &Document,
    scene: SceneId,
    from: SpanRef,
    to: SpanRef,
    count: u32,
    params: FlowParams,
) -> Result<BitmapFrames, BitmapInbetweenError> {
    if !(1..=MAX_BITMAP_INBETWEENS).contains(&count) {
        return Err(BitmapInbetweenError::Count(count));
    }
    let first = keyframe_bitmap(document, scene, from)?;
    let second = keyframe_bitmap(document, scene, to)?;
    let a = Raster::new(first.width(), first.height(), first.pixels())?;
    let b = Raster::new(second.width(), second.height(), second.pixels())?;
    let inbetweener = Inbetweener::new(a, b, params)?;
    let frames = (1..=count)
        .map(|i| inbetweener.frame(f64::from(i) / f64::from(count + 1)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(BitmapFrames {
        width: a.width(),
        height: a.height(),
        frames,
    })
}

/// The pixels of the bitmap a keyframe holds, or why it holds none.
fn keyframe_bitmap(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
) -> Result<&flare_document::Bitmap, BitmapInbetweenError> {
    let span =
        find_span(document, scene, at).ok_or(BitmapInbetweenError::NoSuchKeyframe(at.start))?;
    match span.content() {
        Content::Bitmap(placed) => document
            .bitmap(placed.bitmap())
            .ok_or(BitmapInbetweenError::NotBitmap(at.start)),
        _ => Err(BitmapInbetweenError::NotBitmap(at.start)),
    }
}
