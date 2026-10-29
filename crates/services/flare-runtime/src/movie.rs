//! Exporting the whole animation as a looping GIF, the movie format that needs no encoder beyond
//! a library: every frame of the first scene, scaled down to fit a size limit.

use flare_document::Document;

/// Longest side of the movie in pixels; bigger stages are scaled down to it.
pub const MAX_MOVIE_SIDE: u32 = 640;
/// Most frames a movie holds.
pub const MAX_MOVIE_FRAMES: u64 = 2000;

/// Renders every frame of the first scene and encodes them as an endlessly looping GIF at the
/// document's frame rate (GIF delays are in hundredths of a second, so rates round to those).
/// Errors are worded for the person who asked for the export.
pub fn gif_bytes(document: &Document) -> Result<Vec<u8>, String> {
    let scene = document
        .scenes()
        .first()
        .ok_or("The document has no scene to export.")?;
    let frames = scene.timeline().frame_count().max(1);
    if frames > MAX_MOVIE_FRAMES {
        return Err(format!(
            "The movie has {frames} frames; a GIF is limited to {MAX_MOVIE_FRAMES}."
        ));
    }
    let stage = document.stage();
    let (stage_w, stage_h) = (f64::from(stage.width()), f64::from(stage.height()));
    let scale = (f64::from(MAX_MOVIE_SIDE) / stage_w.max(stage_h)).min(1.0);
    let (width, height) = (
        ((stage_w * scale).round() as u32).max(1),
        ((stage_h * scale).round() as u32).max(1),
    );
    let side = |n: u32| u16::try_from(n).map_err(|_| "The movie is too large.".to_owned());
    let rate = document.frame_rate();
    let fps = f64::from(rate.numerator()) / f64::from(rate.denominator());
    if !fps.is_finite() || fps <= 0.0 {
        return Err("The frame rate must be above zero to export a movie.".to_owned());
    }
    let delay = ((100.0 / fps).round() as u16).max(2);

    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut bytes, side(width)?, side(height)?, &[])
            .map_err(|e| format!("Cannot start the GIF: {e}"))?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|e| format!("Cannot start the GIF: {e}"))?;
        let viewport = flare_render::Viewport {
            x: 0.0,
            y: 0.0,
            width: stage_w,
            height: stage_h,
        };
        for frame in 0..frames as u32 {
            let drawn = flare_eval::evaluate_frame(document, scene.id(), frame)
                .map_err(|e| format!("Cannot draw frame {}: {e}", frame + 1))?;
            // A GIF has no partial transparency, so frames are drawn over the stage color.
            let background = {
                let c = stage.background();
                flare_core::Rgba8::opaque(c.r, c.g, c.b)
            };
            let image = flare_render::render_viewport(&drawn, &viewport, width, height, background)
                .map_err(|e| format!("Cannot render frame {}: {e}", frame + 1))?;
            let mut rgba = image.rgba().to_vec();
            let mut picture =
                gif::Frame::from_rgba_speed(side(width)?, side(height)?, &mut rgba, 10);
            picture.delay = delay;
            encoder
                .write_frame(&picture)
                .map_err(|e| format!("Cannot write frame {}: {e}", frame + 1))?;
        }
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
