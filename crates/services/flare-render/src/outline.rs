//! Drawings as lines: the onion-skin outlines and Outline Mode.

use flare_core::Rgba8;
use flare_document::{Shape, Stroke as Stroke2};
use flare_eval::Frame;

use crate::{Image, RenderError, Viewport, render_viewport};

/// `frame` as outlines only: each shape becomes its path stroked `line_width` wide (in the
/// shape's own units, so it stays about that wide however it is scaled) with no fill. These are
/// what onion-skin outlines draw instead of translucent copies.
fn outlined(frame: &Frame<'_>, line_width: f64, color: Option<Rgba8>) -> Vec<Shape> {
    frame
        .shapes
        .iter()
        .map(|drawn| {
            let scale = drawn.transform.matrix2.determinant().abs().sqrt().max(1e-6);
            let width = (line_width / scale).clamp(1e-6, flare_document::MAX_STROKE_WIDTH);
            let path = drawn.shape.path().to_vec();
            match (
                Shape::new(drawn.shape.id(), path, Rgba8::TRANSPARENT),
                Stroke2::new(width, color.unwrap_or(drawn.outline)),
            ) {
                (Ok(shape), Ok(stroke)) => shape.with_stroke(stroke),
                _ => drawn.shape.as_ref().clone(),
            }
        })
        .collect()
}

/// [`render_onion_viewport`] with the ghosts drawn as thin outlines rather than faded pictures,
/// `line_width` stage units wide, so earlier and later frames show as guide lines over the
/// current one.
pub fn render_onion_outlines_viewport(
    ghosts: &[(&Frame<'_>, f32)],
    current: &Frame<'_>,
    viewport: &Viewport,
    width: u32,
    height: u32,
    background: Rgba8,
    line_width: f64,
) -> Result<Image, RenderError> {
    let empty = Frame::default();
    let mut image = render_viewport(&empty, viewport, width, height, background)?;
    for (frame, opacity) in ghosts {
        let shapes = outlined(frame, line_width, Some(Rgba8::opaque(70, 150, 255)));
        let lines = as_lines(frame, &shapes, true);
        let ghost = render_viewport(&lines, viewport, width, height, Rgba8::TRANSPARENT)?;
        image.composite(&ghost.faded(opacity.max(0.35)))?;
    }
    image.composite(&render_viewport(
        current,
        viewport,
        width,
        height,
        Rgba8::TRANSPARENT,
    )?)?;
    Ok(image)
}

/// `frame`'s shapes replaced by `shapes`, same placement. Masks are kept only when `clips` is set.
fn as_lines<'a>(frame: &Frame<'a>, shapes: &'a [Shape], clips: bool) -> Frame<'a> {
    Frame {
        placed: Vec::new(),
        shapes: frame
            .shapes
            .iter()
            .zip(shapes)
            .map(|(drawn, shape)| flare_eval::DrawnShape {
                path: drawn.path.clone(),
                shape: std::borrow::Cow::Borrowed(shape),
                phase: drawn.phase,
                transform: drawn.transform,
                clips: if clips {
                    drawn.clips.clone()
                } else {
                    Vec::new()
                },
                outline: drawn.outline,
            })
            .collect(),
        clips: if clips {
            frame.clips.clone()
        } else {
            Vec::new()
        },
    }
}

/// Outline Mode: every shape of `frame` as a `line_width` line (stage units) in the color of the
/// layer it is on, no fills. Masks do not hide anything, as in Animate, so everything can be found.
pub fn render_outline_mode_viewport(
    frame: &Frame<'_>,
    viewport: &Viewport,
    width: u32,
    height: u32,
    background: Rgba8,
    line_width: f64,
) -> Result<Image, RenderError> {
    let shapes = outlined(frame, line_width, None);
    render_viewport(
        &as_lines(frame, &shapes, false),
        viewport,
        width,
        height,
        background,
    )
}
