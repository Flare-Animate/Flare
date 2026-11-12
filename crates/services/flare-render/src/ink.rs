//! Where the shapes of a frame leave ink: what the Paint Bucket looks for the edges of an area in.

use flare_eval::Frame;
use vello_cpu::kurbo::{Affine, Shape as _};

use crate::paint::bez_path;
use crate::render::render_viewport;
use crate::{RenderError, Viewport};
use flare_core::Rgba8;

/// How opaque a pixel must be to count as ink. Low, so the faint edge of a thin line still
/// closes an area rather than letting it leak through.
const INK_ALPHA: u8 = 24;

/// The stage rectangle the shapes `frame.shapes[from..]` draw in, strokes included, or `None`
/// when they draw nothing anywhere.
pub fn ink_bounds(frame: &Frame<'_>, from: usize) -> Option<Viewport> {
    let mut bounds: Option<vello_cpu::kurbo::Rect> = None;
    for drawn in frame.shapes.get(from..)? {
        if drawn.shape.fill_is_invisible() && drawn.shape.stroke().is_none() {
            continue;
        }
        // A whole width rather than half, so a mitred corner is inside too.
        let reach = drawn.shape.stroke().map_or(0.0, |s| s.width());
        let local = bez_path(drawn.shape.path())
            .bounding_box()
            .inflate(reach, reach);
        let m = drawn.transform;
        let affine = Affine::new([
            m.matrix2.x_axis.x,
            m.matrix2.x_axis.y,
            m.matrix2.y_axis.x,
            m.matrix2.y_axis.y,
            m.translation.x,
            m.translation.y,
        ]);
        let placed = affine.transform_rect_bbox(local);
        if ![placed.x0, placed.y0, placed.x1, placed.y1]
            .iter()
            .all(|v| v.is_finite())
        {
            continue;
        }
        bounds = Some(bounds.map_or(placed, |b| b.union(placed)));
    }
    let b = bounds?;
    Some(Viewport {
        x: b.x0,
        y: b.y0,
        width: b.width(),
        height: b.height(),
    })
    .filter(|v| v.width > 0.0 && v.height > 0.0)
}

/// Which pixels of a `width` by `height` picture of `area` the shapes `frame.shapes[from..]`
/// leave ink on, row by row. Pictures and the stage background are not ink.
pub fn ink_mask(
    frame: &Frame<'_>,
    from: usize,
    area: &Viewport,
    width: u32,
    height: u32,
) -> Result<Vec<bool>, RenderError> {
    let above = Frame {
        placed: frame.placed.clone(),
        shapes: frame.shapes.get(from..).unwrap_or_default().to_vec(),
        bitmaps: Vec::new(),
        clips: frame.clips.clone(),
        warnings: Vec::new(),
    };
    let image = render_viewport(&above, area, width, height, Rgba8::TRANSPARENT)?;
    Ok(image
        .rgba
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| p[3] >= INK_ALPHA)
        .collect())
}
