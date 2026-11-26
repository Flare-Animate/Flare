//! A picture of the mark the Brush makes on a raster layer, for the list of its presets: one pen
//! stroke laid by the stroke engine itself, so the picture is exactly what the brush paints.

use std::sync::Arc;

use flare_core::{LayerId, Rgba8};
use glamx::DAffine2;

use super::canvas::Canvas;
use super::paint::Painting;
use super::{BlendError, BlendKind, BlendSettings, BlendStroke, InputSample};
use crate::brush::swatch_samples;
use crate::common::SpanRef;

/// The largest side of a swatch, in pixels.
pub const MAX_SWATCH_SIDE: u32 = 1024;

/// The band a brush that mixes is drawn across: a grey between the paint (white) and nothing, so
/// once the swatch is tinted the smear still reads.
const BAND: [u8; 4] = [104, 104, 104, 255];

/// The Brush's mark with `settings`, `width` by `height` pixels, straight RGBA in white, to be
/// tinted where it is shown as the vector Brush's swatches are. The stroke is the vector list's
/// own sample (`brush::swatch_samples`), laid inside `pad` pixels of the edges, so both lists
/// show the same S with the same swell. A brush that mixes is drawn across a grey band, which it
/// smears; a plain one over nothing. The brush's size is scaled to the swatch: a pencil stays
/// thinner than a round brush, an airbrush wider, all within the swatch.
pub fn raster_swatch(
    settings: BlendSettings,
    width: u32,
    height: u32,
    pad: u32,
) -> Result<Vec<u8>, BlendError> {
    settings.validate()?;
    if !(8..=MAX_SWATCH_SIDE).contains(&width)
        || !(8..=MAX_SWATCH_SIDE).contains(&height)
        || pad.saturating_mul(2) >= width.min(height)
    {
        return Err(BlendError::Size);
    }
    let (w, h) = (width as usize, height as usize);
    let mut ground = vec![0u8; w * h * 4];
    if settings.mix > 0.0 {
        let (left, right) = (pad as usize * 4, (w - pad as usize) * 4);
        for row in h * 3 / 8..h - h * 3 / 8 {
            for px in ground[row * w * 4 + left..row * w * 4 + right]
                .as_chunks_mut::<4>()
                .0
            {
                px.copy_from_slice(&BAND);
            }
        }
    }
    let side = height as f32;
    // A 20-unit brush, where most presets sit, takes a quarter of the swatch's height.
    let diameter = (settings.size * side / 80.0).clamp(1.5, side * 0.6);
    let canvas = Canvas::new(width, height, Arc::from(ground), None);
    let nowhere = SpanRef {
        layer: LayerId::new(),
        start: 0,
        element: 0,
    };
    let mut stroke = BlendStroke::new(
        BlendKind::RasterBrush,
        settings,
        nowhere,
        None,
        String::new(),
        DAffine2::IDENTITY,
        diameter / settings.size,
        canvas,
    );
    stroke.painting = Some(Painting::new(Rgba8::new(255, 255, 255, 255)));
    let inner = |v: u32| f64::from(v - 2 * pad);
    let samples: Vec<InputSample> = swatch_samples(inner(width), inner(height))
        .into_iter()
        .map(|mut s| {
            s.x += f64::from(pad);
            s.y += f64::from(pad);
            s
        })
        .collect();
    stroke.extend(&samples);
    Ok(stroke.canvas.working)
}
