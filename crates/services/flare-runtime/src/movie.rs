//! Exporting the whole animation as a looping GIF, the movie format that needs no encoder beyond
//! a library: every frame of the first scene, scaled down to fit a size limit.
//!
//! The frames do not depend on each other, so on a desktop they are drawn and quantized several
//! at a time, on threads of their own that leave one core free for the window, and written in
//! order. The result is the same bytes whatever the number of threads.

use flare_core::{Rgba8, SceneId};
use flare_document::Document;
use flare_render::Viewport;

/// Longest side of the movie in pixels; bigger stages are scaled down to it.
pub const MAX_MOVIE_SIDE: u32 = 640;
/// Most frames a movie holds.
pub const MAX_MOVIE_FRAMES: u64 = 2000;

/// How many frames are made at once: every core but one, at most eight, and one where there are
/// no threads to give (the browser).
fn workers() -> usize {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::available_parallelism()
            .map_or(1, |n| n.get().saturating_sub(1))
            .clamp(1, 8)
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
}

/// Renders every frame of the first scene and encodes them as an endlessly looping GIF at the
/// document's frame rate (GIF delays are in hundredths of a second, so rates round to those).
/// Errors are worded for the person who asked for the export.
pub fn gif_bytes(document: &Document) -> Result<Vec<u8>, String> {
    gif_bytes_on(document, workers())
}

/// [`gif_bytes`] with frames made `workers` at a time.
pub fn gif_bytes_on(document: &Document, workers: usize) -> Result<Vec<u8>, String> {
    let plan = plan(document)?;
    check_frames(&plan, "GIF")?;
    let side = |n: u32| u16::try_from(n).map_err(|_| "The movie is too large.".to_owned());
    let (gif_w, gif_h) = (side(plan.width)?, side(plan.height)?);
    let delay = ((100.0 / plan.fps).round() as u16).max(2);
    let frames = plan.frames;
    let picture = |frame: u32| -> Result<gif::Frame<'static>, String> {
        let image = draw(document, &plan, frame)?;
        let mut rgba = image.rgba().to_vec();
        let mut picture = gif::Frame::from_rgba_speed(gif_w, gif_h, &mut rgba, 10);
        picture.delay = delay;
        Ok(picture)
    };

    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut bytes, gif_w, gif_h, &[])
            .map_err(|e| format!("Cannot start the GIF: {e}"))?;
        encoder
            .set_repeat(gif::Repeat::Infinite)
            .map_err(|e| format!("Cannot start the GIF: {e}"))?;
        let make = Maker::new(workers);
        // A few batches of frames in flight, never the whole movie: memory stays bounded.
        let batch = (workers.max(1) * 2) as u32;
        let mut start = 0u32;
        while u64::from(start) < frames {
            let end = (start + batch).min(frames as u32);
            for (offset, made) in make.run(start..end, &picture)?.into_iter().enumerate() {
                encoder.write_frame(&made).map_err(|e| {
                    format!("Cannot write frame {}: {e}", start as usize + offset + 1)
                })?;
            }
            start = end;
        }
    }
    Ok(bytes)
}

/// What an animation export draws: every frame of the first scene at one size, over the stage
/// colour (the animation formats here have no partial transparency).
struct Plan {
    scene: SceneId,
    frames: u64,
    width: u32,
    height: u32,
    fps: f64,
    viewport: Viewport,
    background: Rgba8,
}

fn plan(document: &Document) -> Result<Plan, String> {
    let scene = document
        .scenes()
        .first()
        .ok_or("The document has no scene to export.")?;
    let frames = scene.timeline().frame_count().max(1);
    let stage = document.stage();
    let (stage_w, stage_h) = (f64::from(stage.width()), f64::from(stage.height()));
    let scale = (f64::from(MAX_MOVIE_SIDE) / stage_w.max(stage_h)).min(1.0);
    let (width, height) = (
        ((stage_w * scale).round() as u32).max(1),
        ((stage_h * scale).round() as u32).max(1),
    );
    let fps = document.frame_rate().per_second();
    if !fps.is_finite() || fps <= 0.0 {
        return Err("The frame rate must be above zero to export a movie.".to_owned());
    }
    let viewport = Viewport {
        x: 0.0,
        y: 0.0,
        width: stage_w,
        height: stage_h,
    };
    let background = {
        let c = stage.background();
        Rgba8::opaque(c.r, c.g, c.b)
    };
    Ok(Plan {
        scene: scene.id(),
        frames,
        width,
        height,
        fps,
        viewport,
        background,
    })
}

fn check_frames(plan: &Plan, format: &str) -> Result<(), String> {
    if plan.frames > MAX_MOVIE_FRAMES {
        return Err(format!(
            "The movie has {} frames; {format} is limited to {MAX_MOVIE_FRAMES}.",
            plan.frames
        ));
    }
    Ok(())
}

/// Frame `frame` of the plan, straight alpha, at the plan's size.
fn draw(document: &Document, plan: &Plan, frame: u32) -> Result<flare_render::Image, String> {
    let drawn = flare_eval::evaluate_frame(document, plan.scene, frame)
        .map_err(|e| format!("Cannot draw frame {}: {e}", frame + 1))?;
    flare_render::render_viewport(
        &drawn,
        &plan.viewport,
        plan.width,
        plan.height,
        plan.background,
    )
    .map_err(|e| format!("Cannot render frame {}: {e}", frame + 1))
}

/// Renders every frame of the first scene and encodes them as an endlessly looping APNG (animated
/// PNG) at the document's frame rate. A PNG frame holds milliseconds, so the rate rounds to those.
pub fn apng_bytes(document: &Document) -> Result<Vec<u8>, String> {
    let plan = plan(document)?;
    check_frames(&plan, "APNG")?;
    let start = |e: &dyn std::fmt::Display| format!("Cannot start the APNG: {e}");
    let delay_ms = ((1000.0 / plan.fps).round() as u16).max(1);
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, plan.width, plan.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .set_animated(plan.frames as u32, 0)
        .map_err(|e| start(&e))?;
    let mut writer = encoder.write_header().map_err(|e| start(&e))?;
    for frame in 0..plan.frames as u32 {
        writer
            .set_frame_delay(delay_ms, 1000)
            .map_err(|e| start(&e))?;
        let image = draw(document, &plan, frame)?;
        writer
            .write_image_data(image.rgba())
            .map_err(|e| format!("Cannot write frame {}: {e}", frame + 1))?;
    }
    writer
        .finish()
        .map_err(|e| format!("Cannot finish the APNG: {e}"))?;
    Ok(bytes)
}

/// Makes a batch of frames, in order, on the pool or on the calling thread.
struct Maker {
    #[cfg(not(target_arch = "wasm32"))]
    pool: Option<rayon::ThreadPool>,
}

impl Maker {
    fn new(workers: usize) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let pool = (workers > 1)
                .then(|| {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(workers)
                        .thread_name(|i| format!("flare-export-{i}"))
                        .build()
                        .ok()
                })
                .flatten();
            Self { pool }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = workers;
            Self {}
        }
    }

    fn run<T: Send>(
        &self,
        frames: std::ops::Range<u32>,
        make: &(impl Fn(u32) -> Result<T, String> + Sync),
    ) -> Result<Vec<T>, String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(pool) = &self.pool {
            use rayon::prelude::*;
            return pool.install(|| frames.into_par_iter().map(make).collect());
        }
        frames.map(make).collect()
    }
}

#[cfg(test)]
mod tests;
