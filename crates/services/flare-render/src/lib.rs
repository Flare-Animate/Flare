//! CPU rasterization of an evaluated frame with vello_cpu.
//!
//! Document units map 1:1 to pixels with the origin at the top-left. Output is straight
//! (non-premultiplied) 8-bit sRGB RGBA. This crate owns only drawing; timeline evaluation
//! lives in `flare-eval` and the file formats elsewhere.
#![forbid(unsafe_code)]

use std::io::Write;

use flare_core::Rgba8;
use flare_document::{PathSegment, Shape, Stroke as Stroke2};
use flare_eval::Frame;
use thiserror::Error;
use vello_cpu::color::{AlphaColor, Srgb};
use vello_cpu::kurbo::{Affine, BezPath, Cap, Join, Point, Rect, Shape as _, Stroke};
use vello_cpu::peniko::{Gradient, ImageAlphaType};
use vello_cpu::{Pixmap, RenderContext, Resources};

/// Per-side limit (vello_cpu uses `u16`; we stay well below).
pub const MAX_DIMENSION: u32 = 16_384;
/// Total pixel budget, so a huge request cannot exhaust memory.
pub const MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error(
        "image size {0}x{1} is invalid (each side 1..={MAX_DIMENSION}, at most {MAX_PIXELS} pixels)"
    )]
    InvalidSize(u32, u32),
    #[error("png encoding failed: {0}")]
    Png(#[from] png::EncodingError),
}

/// Premultiplied-alpha RGBA8 pixels, row-major from the top-left: what the rasterizer produces
/// and what a GPU or egui texture wants, so showing a frame needs no conversion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PremultipliedImage {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl PremultipliedImage {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

/// Straight-alpha RGBA8 pixels, row-major from the top-left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Image {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<Rgba8> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let p = &self.rgba[i..i + 4];
        Some(Rgba8::new(p[0], p[1], p[2], p[3]))
    }

    /// A copy with every pixel's alpha multiplied by `opacity` (clamped to 0..=1). Colors are
    /// unchanged because the pixels are straight alpha.
    pub fn faded(&self, opacity: f32) -> Image {
        let opacity = opacity.clamp(0.0, 1.0);
        let mut out = self.clone();
        for px in bytemuck::cast_slice_mut::<u8, [u8; 4]>(&mut out.rgba) {
            px[3] = (f32::from(px[3]) * opacity).round() as u8;
        }
        out
    }

    /// Draws `top` over this image (source-over, straight alpha). Sizes must match.
    pub fn composite(&mut self, top: &Image) -> Result<(), RenderError> {
        if (self.width, self.height) != (top.width, top.height) {
            return Err(RenderError::InvalidSize(top.width, top.height));
        }
        let pairs = self
            .rgba
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(top.rgba.as_chunks::<4>().0);
        for (bottom, over) in pairs {
            let (ta, ba) = (u32::from(over[3]), u32::from(bottom[3]));
            if ta == 0 {
                continue;
            }
            // out_a = ta + ba (1 - ta), in 0..=255 fixed point with rounding.
            let out_a = ta * 255 + ba * (255 - ta);
            let out_a_scaled = ((out_a + 127) / 255).min(255);
            for channel in 0..3 {
                let top_part = u32::from(over[channel]) * ta * 255;
                let bottom_part = u32::from(bottom[channel]) * ba * (255 - ta);
                bottom[channel] =
                    ((top_part + bottom_part + out_a / 2) / out_a.max(1)).min(255) as u8;
            }
            bottom[3] = out_a_scaled as u8;
        }
        Ok(())
    }

    pub fn write_png(&self, writer: impl Write) -> Result<(), RenderError> {
        let mut encoder = png::Encoder::new(writer, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header()?.write_image_data(&self.rgba)?;
        Ok(())
    }
}

fn color(c: Rgba8) -> AlphaColor<Srgb> {
    AlphaColor::from_rgba8(c.r, c.g, c.b, c.a)
}

fn linear_gradient(gradient: &flare_document::Gradient) -> Gradient {
    let (from, to) = (gradient.from().as_vec(), gradient.to().as_vec());
    Gradient::new_linear((from.x, from.y), (to.x, to.y))
        .with_stops([color(gradient.start()), color(gradient.end())])
}

/// The region of mask `clip`, in scene coordinates: the union of its shapes (a shape drawn
/// with the opposite winding over another one cuts a hole, as with any single filled path).
fn clip_path(clip: &[flare_eval::ClipShape<'_>]) -> BezPath {
    let mut path = BezPath::new();
    for member in clip {
        let [a, b, c, d, e, f] = member.transform.to_cols_array();
        let mut one = bez_path(member.shape.path());
        one.apply_affine(Affine::new([a, b, c, d, e, f]));
        path.extend(one.elements().iter().copied());
    }
    path
}

fn bez_path(segments: &[PathSegment]) -> BezPath {
    let mut path = BezPath::new();
    for segment in segments {
        match segment {
            PathSegment::MoveTo(p) => path.move_to((p.as_vec().x, p.as_vec().y)),
            PathSegment::LineTo(p) => path.line_to((p.as_vec().x, p.as_vec().y)),
            PathSegment::CubicTo(a, b, c) => path.curve_to(
                (a.as_vec().x, a.as_vec().y),
                (b.as_vec().x, b.as_vec().y),
                (c.as_vec().x, c.as_vec().y),
            ),
            PathSegment::Close => path.close_path(),
        }
    }
    path
}

/// The line style `stroke` asks for, `width` wide.
fn stroke_style(width: f64, stroke: flare_document::Stroke) -> Stroke {
    use flare_document::{Cap as DocCap, Join as DocJoin};
    let caps = match stroke.cap() {
        DocCap::Butt => Cap::Butt,
        DocCap::Round => Cap::Round,
        DocCap::Square => Cap::Square,
    };
    let join = match stroke.join() {
        DocJoin::Round => Join::Round,
        DocJoin::Bevel => Join::Bevel,
        DocJoin::Miter => Join::Miter,
    };
    // A miter that would stick out more than four widths is cut off, as other renderers do.
    Stroke::new(width)
        .with_caps(caps)
        .with_join(join)
        .with_miter_limit(4.0)
}

/// Renders `current` with translucent `ghosts` (frame, opacity) beneath it, as onion
/// skinning does. Ghosts are drawn in the order given, then the current frame on top.
pub fn render_onion(
    ghosts: &[(&Frame<'_>, f32)],
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
    ghosts: &[(&Frame<'_>, f32)],
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
    for (frame, opacity) in ghosts {
        let ghost = render_viewport(frame, viewport, width, height, Rgba8::TRANSPARENT)?;
        image.composite(&ghost.faded(*opacity))?;
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

/// `frame` as outlines only: each shape becomes its path stroked `line_width` wide (in the
/// shape's own units, so it stays about that wide however it is scaled) with no fill. These are
/// what onion-skin outlines draw instead of translucent copies.
fn outlined(frame: &Frame<'_>, line_width: f64, color: Rgba8) -> Vec<Shape> {
    frame
        .shapes
        .iter()
        .map(|drawn| {
            let scale = drawn.transform.matrix2.determinant().abs().sqrt().max(1e-6);
            let width = (line_width / scale).clamp(1e-6, flare_document::MAX_STROKE_WIDTH);
            let path = drawn.shape.path().to_vec();
            match (
                Shape::new(drawn.shape.id(), path, Rgba8::TRANSPARENT),
                Stroke2::new(width, color),
            ) {
                (Ok(shape), Ok(stroke)) => shape.with_stroke(stroke),
                _ => drawn.shape.clone(),
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
        let shapes = outlined(frame, line_width, Rgba8::opaque(70, 150, 255));
        let lines = Frame {
            placed: Vec::new(),
            shapes: frame
                .shapes
                .iter()
                .zip(&shapes)
                .map(|(drawn, shape)| flare_eval::DrawnShape {
                    path: drawn.path.clone(),
                    shape,
                    transform: drawn.transform,
                    clips: drawn.clips.clone(),
                })
                .collect(),
            clips: frame.clips.clone(),
        };
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

/// Indices into `frame.shapes` of every shape whose fill or stroke contains `point` (scene
/// coordinates), topmost first. Fills use the same non-zero rule as rendering; a stroke counts
/// where it is drawn (round caps and joins, width scaled by the shape's transform), so a line
/// or an outline-only shape can be picked. Fully transparent fills without a stroke and
/// singular transforms are not hittable. Callers that must skip some shapes (locked layers)
/// walk this list instead of taking only the first.
pub fn hit_test_all(frame: &Frame<'_>, point: glamx::DVec2) -> Vec<usize> {
    let clip_paths: Vec<BezPath> = frame.clips.iter().map(|clip| clip_path(clip)).collect();
    let scene_point = Point::new(point.x, point.y);
    frame
        .shapes
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, drawn)| {
            // Masked artwork can only be picked where it is actually visible.
            if drawn.clips.iter().any(|index| {
                clip_paths
                    .get(*index)
                    .is_none_or(|p| !p.contains(scene_point))
            }) {
                return false;
            }
            let determinant = drawn.transform.matrix2.determinant();
            if determinant == 0.0 || !determinant.is_finite() {
                return false;
            }
            let local = drawn.transform.inverse().transform_point2(point);
            let local = Point::new(local.x, local.y);
            let path = bez_path(drawn.shape.path());
            if !drawn.shape.fill_is_invisible() && path.contains(local) {
                return true;
            }
            drawn.shape.stroke().is_some_and(|stroke| {
                // Flatten finely relative to how big a local unit is on screen.
                let tolerance = 0.1 / determinant.abs().sqrt().max(1e-6);
                let style = stroke_style(stroke.width(), stroke);
                let outline = vello_cpu::kurbo::stroke(
                    path.elements().iter().copied(),
                    &style,
                    &vello_cpu::kurbo::StrokeOpts::default(),
                    tolerance,
                );
                outline.contains(local)
            })
        })
        .map(|(index, _)| index)
        .collect()
}

/// Index into `frame.shapes` of the topmost shape under `point`; see [`hit_test_all`].
pub fn hit_test(frame: &Frame<'_>, point: glamx::DVec2) -> Option<usize> {
    hit_test_all(frame, point).into_iter().next()
}

/// Converts premultiplied RGBA8 to straight alpha with rounding, in place. Opaque and fully
/// transparent pixels are already the same in both forms and are not touched.
fn unpremultiply_in_place(rgba: &mut [u8]) {
    for px in bytemuck::cast_slice_mut::<u8, [u8; 4]>(rgba) {
        let a = u32::from(px[3]);
        if a == 0 || a == 255 {
            continue;
        }
        for c in &mut px[..3] {
            *c = ((u32::from(*c) * 255 + a / 2) / a).min(255) as u8;
        }
    }
}

/// A rectangle of the stage, in stage units, to draw into an image of any size.
///
/// Rendering only what is on screen, at the resolution it is shown, keeps the work and the
/// memory proportional to the window rather than to the stage, and keeps zoomed-in views sharp.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Viewport {
    /// The whole stage at one image pixel per stage unit.
    pub fn full(width: u32, height: u32) -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: f64::from(width),
            height: f64::from(height),
        }
    }

    fn is_valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|v| v.is_finite())
            && self.width > 0.0
            && self.height > 0.0
    }
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
const PARALLEL_MIN_SHAPES: usize = 800;
/// More workers than this stopped helping in the same measurements (20 000 shapes: 8.6 ms with
/// four, 9.2 ms with seven), and a drawing program should leave cores for the rest of the app.
const MAX_RENDER_THREADS: u16 = 4;

/// Worker threads to give the rasterizer for a frame of `shapes` shapes on a machine with
/// `cores` cores. Zero means draw on the calling thread.
fn render_threads(shapes: usize, cores: usize) -> u16 {
    if shapes < PARALLEL_MIN_SHAPES || cores < 3 {
        return 0;
    }
    // One core stays free for the caller, which also takes part in the work.
    u16::try_from(cores - 1).map_or(MAX_RENDER_THREADS, |n| n.min(MAX_RENDER_THREADS))
}

fn available_cores() -> usize {
    static CORES: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *CORES.get_or_init(|| std::thread::available_parallelism().map_or(1, usize::from))
}

fn rasterize(
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
    for drawn in &frame.shapes {
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
            ctx.stroke_path(&path);
        }
        for _ in 0..pushed {
            ctx.pop_layer();
        }
    }
    ctx.flush();
    let mut pixmap = Pixmap::new(w16, h16);
    ctx.render(&mut pixmap, &mut Resources::new());
    Ok(pixmap)
}

#[cfg(test)]
mod tests;
