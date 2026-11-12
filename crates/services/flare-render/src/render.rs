//! Rasterizing a frame, whole or in part, with onion-skin ghosts or without.

use flare_core::Rgba8;
use flare_eval::Frame;
use vello_cpu::kurbo::{Affine, BezPath, Rect};
use vello_cpu::peniko::ImageAlphaType;
use vello_cpu::peniko::{Extend, ImageBrush, ImageQuality, ImageSampler};
use vello_cpu::{ImageSource, Pixmap, RenderContext, Resources};

#[allow(unused_imports)]
use crate::image::unpremultiply_in_place;
#[allow(unused_imports)]
use crate::paint::{bez_path, clip_path, color, linear_gradient, stroke_style};
use crate::pattern::pattern;
#[allow(unused_imports)]
use crate::{Image, MAX_DIMENSION, MAX_PIXELS, PremultipliedImage, RenderError, Viewport};

/// Renders `current` with translucent `ghosts` (frame, opacity, tint) beneath it, as onion
/// skinning does. Ghosts are drawn in the order given, then the current frame on top; a ghost
/// with a tint is colored with it ([`Image::tinted`]), as Animate's tinted onion skin is.
pub fn render_onion(
    ghosts: &[(&Frame<'_>, f32, Option<Rgba8>)],
    current: &Frame<'_>,
    width: u32,
    height: u32,
    background: Rgba8,
) -> Result<Image, RenderError> {
    render_onion_viewport(
        ghosts,
        current,
        &Viewport::full(width, height),
        width,
        height,
        background,
    )
}

/// [`render_onion`] for a part of the stage; see [`render_viewport`].
pub fn render_onion_viewport(
    ghosts: &[(&Frame<'_>, f32, Option<Rgba8>)],
    current: &Frame<'_>,
    viewport: &Viewport,
    width: u32,
    height: u32,
    background: Rgba8,
) -> Result<Image, RenderError> {
    if ghosts.is_empty() {
        return render_viewport(current, viewport, width, height, background);
    }
    let empty = Frame::default();
    let mut image = render_viewport(&empty, viewport, width, height, background)?;
    for (frame, opacity, tint) in ghosts {
        // The ghost is drawn only to be colored and faded, so both are done on it in place.
        let mut ghost = render_viewport(frame, viewport, width, height, Rgba8::TRANSPARENT)?;
        if let Some(color) = tint {
            ghost.tint_in_place(*color);
        }
        ghost.fade_in_place(*opacity);
        image.composite(&ghost)?;
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

/// Rasterizes `frame` at `width` x `height` over `background` (use
/// [`Rgba8::TRANSPARENT`] for no background).
pub fn render(
    frame: &Frame<'_>,
    width: u32,
    height: u32,
    background: Rgba8,
) -> Result<Image, RenderError> {
    render_viewport(
        frame,
        &Viewport::full(width, height),
        width,
        height,
        background,
    )
}

/// Rasterizes the part of `frame` inside `viewport`, scaled to `width` x `height` pixels.
pub fn render_viewport(
    frame: &Frame<'_>,
    viewport: &Viewport,
    width: u32,
    height: u32,
    background: Rgba8,
) -> Result<Image, RenderError> {
    let pixmap = rasterize(frame, viewport, width, height, background)?;
    // An image with no translucent pixel is the same in both alpha forms; the rasterizer knows.
    let translucent = pixmap.may_have_transparency();
    let mut rgba = pixmap.take_rgba8(ImageAlphaType::AlphaPremultiplied);
    if translucent {
        unpremultiply_in_place(&mut rgba);
    }
    Ok(Image {
        width,
        height,
        rgba,
    })
}

/// Like [`render_viewport`] but leaves the pixels premultiplied, as the rasterizer made them.
/// This is the form to hand to a texture: it skips a pass over every pixel here and the matching
/// premultiplying pass on the receiving side.
pub fn render_viewport_premultiplied(
    frame: &Frame<'_>,
    viewport: &Viewport,
    width: u32,
    height: u32,
    background: Rgba8,
) -> Result<PremultipliedImage, RenderError> {
    let pixmap = rasterize(frame, viewport, width, height, background)?;
    Ok(PremultipliedImage {
        width,
        height,
        rgba: pixmap.take_rgba8(ImageAlphaType::AlphaPremultiplied),
    })
}

/// Below this many shapes a frame is cheaper to draw on one thread: starting workers costs more
/// than it saves (measured: 100 shapes took 0.9 ms alone and 2.0 ms with seven workers).
pub(crate) const PARALLEL_MIN_SHAPES: usize = 800;
/// More workers than this stopped helping in the same measurements (20 000 shapes: 8.6 ms with
/// four, 9.2 ms with seven), and a drawing program should leave cores for the rest of the app.
pub(crate) const MAX_RENDER_THREADS: u16 = 4;

/// Worker threads to give the rasterizer for a frame of `shapes` shapes on a machine with
/// `cores` cores. Zero means draw on the calling thread.
pub(crate) fn render_threads(shapes: usize, cores: usize) -> u16 {
    if shapes < PARALLEL_MIN_SHAPES || cores < 3 {
        return 0;
    }
    // One core stays free for the caller, which also takes part in the work.
    u16::try_from(cores - 1).map_or(MAX_RENDER_THREADS, |n| n.min(MAX_RENDER_THREADS))
}

pub(crate) fn available_cores() -> usize {
    static CORES: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CORES.get_or_init(|| std::thread::available_parallelism().map_or(1, usize::from))
}

pub(crate) fn rasterize(
    frame: &Frame<'_>,
    viewport: &Viewport,
    width: u32,
    height: u32,
    background: Rgba8,
) -> Result<Pixmap, RenderError> {
    if !viewport.is_valid() {
        return Err(RenderError::InvalidSize(width, height));
    }
    let area = u64::from(width) * u64::from(height);
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || area > MAX_PIXELS
    {
        return Err(RenderError::InvalidSize(width, height));
    }
    // Both fit u16 because MAX_DIMENSION < u16::MAX.
    let (w16, h16) = (width as u16, height as u16);
    let threads = render_threads(frame.shapes.len(), available_cores());
    let mut ctx = RenderContext::new_with(
        w16,
        h16,
        vello_cpu::RenderSettings {
            num_threads: threads,
            ..vello_cpu::RenderSettings::default()
        },
    );
    if background.a > 0 {
        ctx.set_paint(color(background));
        ctx.fill_rect(&Rect::new(0.0, 0.0, f64::from(width), f64::from(height)));
    }
    // Stage units to image pixels. Shapes are culled by the rasterizer, so the cost of a zoomed
    // view follows what is visible.
    let base = Affine::scale_non_uniform(
        f64::from(width) / viewport.width,
        f64::from(height) / viewport.height,
    ) * Affine::translate((-viewport.x, -viewport.y));
    let clip_paths: Vec<BezPath> = frame.clips.iter().map(|clip| clip_path(clip)).collect();
    let mut bitmaps = frame.bitmaps.iter().peekable();
    for (index, drawn) in frame.shapes.iter().enumerate() {
        while let Some(bitmap) = bitmaps.next_if(|b| b.after_shapes <= index) {
            draw_bitmap(&mut ctx, base, &clip_paths, bitmap);
        }
        // Masks live in scene space, so they are pushed with the view transform alone, before
        // the shape's own.
        ctx.set_transform(base);
        let mut pushed = 0;
        for index in &drawn.clips {
            if let Some(region) = clip_paths.get(*index) {
                ctx.push_clip_layer(region);
                pushed += 1;
            }
        }
        let [a, b, c, d, e, f] = drawn.transform.to_cols_array();
        ctx.set_transform(base * Affine::new([a, b, c, d, e, f]));
        let path = bez_path(drawn.shape.path());
        match drawn.shape.gradient() {
            Some(gradient) => ctx.set_paint(linear_gradient(&gradient)),
            None => ctx.set_paint(color(drawn.shape.fill())),
        }
        ctx.fill_path(&path);
        if let Some(stroke) = drawn.shape.stroke() {
            ctx.set_stroke(stroke_style(stroke.width(), stroke));
            ctx.set_paint(color(stroke.color()));
            // A geometric style is drawn as the path it builds from the line, not the line.
            match pattern(&path, stroke.width(), stroke.style()) {
                Some(built) => ctx.stroke_path(&built),
                None => ctx.stroke_path(&path),
            }
        }
        for _ in 0..pushed {
            ctx.pop_layer();
        }
    }
    for bitmap in bitmaps {
        draw_bitmap(&mut ctx, base, &clip_paths, bitmap);
    }
    ctx.flush();
    let mut pixmap = Pixmap::new(w16, h16);
    ctx.render(&mut pixmap, &mut Resources::new());
    Ok(pixmap)
}

/// One bitmap, its top-left corner at the origin of its transform. Pixels are premultiplied for
/// the rasterizer here, one pass over the image; the document keeps them straight.
fn draw_bitmap(
    ctx: &mut RenderContext,
    base: Affine,
    clip_paths: &[BezPath],
    drawn: &flare_eval::DrawnBitmap<'_>,
) {
    let bitmap = drawn.bitmap;
    // Both fit u16 because a bitmap's side is at most 8192.
    let (Ok(w), Ok(h)) = (
        u16::try_from(bitmap.width()),
        u16::try_from(bitmap.height()),
    ) else {
        return;
    };
    let mut pixmap = Pixmap::new(w, h);
    for (out, rgba) in pixmap
        .data_mut()
        .iter_mut()
        .zip(bitmap.pixels().as_chunks::<4>().0)
    {
        let a = u16::from(rgba[3]);
        let mul = |c: u8| ((u16::from(c) * a + 127) / 255) as u8;
        out.r = mul(rgba[0]);
        out.g = mul(rgba[1]);
        out.b = mul(rgba[2]);
        out.a = rgba[3];
    }
    ctx.set_transform(base);
    let mut pushed = 0;
    for index in &drawn.clips {
        if let Some(region) = clip_paths.get(*index) {
            ctx.push_clip_layer(region);
            pushed += 1;
        }
    }
    let [a, b, c, d, e, f] = drawn.transform.to_cols_array();
    ctx.set_transform(base * Affine::new([a, b, c, d, e, f]));
    ctx.set_paint(ImageBrush {
        image: ImageSource::Pixmap(std::sync::Arc::new(pixmap)),
        sampler: ImageSampler {
            x_extend: Extend::Pad,
            y_extend: Extend::Pad,
            quality: ImageQuality::Low,
            alpha: drawn.opacity,
        },
    });
    ctx.fill_rect(&Rect::new(0.0, 0.0, f64::from(w), f64::from(h)));
    for _ in 0..pushed {
        ctx.pop_layer();
    }
}
