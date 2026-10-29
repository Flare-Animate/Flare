//! CPU rasterization of an evaluated frame with vello_cpu.
//!
//! Document units map 1:1 to pixels with the origin at the top-left. Output is straight
//! (non-premultiplied) 8-bit sRGB RGBA. This crate owns only drawing; timeline evaluation
//! lives in `flare-eval` and the file formats elsewhere.
#![forbid(unsafe_code)]

use std::io::Write;

use flare_core::Rgba8;
use flare_document::PathSegment;
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
    let empty = Frame {
        placed: Vec::new(),
        shapes: Vec::new(),
    };
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

/// Indices into `frame.shapes` of every shape whose fill or stroke contains `point` (scene
/// coordinates), topmost first. Fills use the same non-zero rule as rendering; a stroke counts
/// where it is drawn (round caps and joins, width scaled by the shape's transform), so a line
/// or an outline-only shape can be picked. Fully transparent fills without a stroke and
/// singular transforms are not hittable. Callers that must skip some shapes (locked layers)
/// walk this list instead of taking only the first.
pub fn hit_test_all(frame: &Frame<'_>, point: glamx::DVec2) -> Vec<usize> {
    frame
        .shapes
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, drawn)| {
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
                let style = Stroke::new(stroke.width())
                    .with_caps(Cap::Round)
                    .with_join(Join::Round);
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
    for drawn in &frame.shapes {
        let [a, b, c, d, e, f] = drawn.transform.to_cols_array();
        ctx.set_transform(base * Affine::new([a, b, c, d, e, f]));
        let path = bez_path(drawn.shape.path());
        match drawn.shape.gradient() {
            Some(gradient) => ctx.set_paint(linear_gradient(&gradient)),
            None => ctx.set_paint(color(drawn.shape.fill())),
        }
        ctx.fill_path(&path);
        if let Some(stroke) = drawn.shape.stroke() {
            ctx.set_stroke(
                Stroke::new(stroke.width())
                    .with_caps(Cap::Round)
                    .with_join(Join::Round),
            );
            ctx.set_paint(color(stroke.color()));
            ctx.stroke_path(&path);
        }
    }
    ctx.flush();
    let mut pixmap = Pixmap::new(w16, h16);
    ctx.render(&mut pixmap, &mut Resources::new());
    Ok(pixmap)
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::num::NonZeroU32;

    use flare_core::InstanceId;
    use flare_core::{DocumentId, FrameRate, LayerId, SceneId, ShapeId, SymbolId, Transform2D};
    use flare_document::{
        Content, Document, FrameSpan, Instance, Layer, Scene, Shape, Symbol, Timeline,
    };
    use glamx::{DAffine2, DVec2};

    use super::*;

    const RED: Rgba8 = Rgba8::opaque(255, 0, 0);

    fn span(content: Content) -> FrameSpan {
        FrameSpan::new(0, NonZeroU32::new(1).expect("nonzero"), content)
    }

    fn scene_with(spans: Vec<FrameSpan>, symbols: Vec<Symbol>) -> (Document, SceneId) {
        let layers = spans
            .into_iter()
            .map(|s| Layer::new(LayerId::new(), "L", vec![s]).expect("layer"))
            .collect();
        let id = SceneId::new();
        let scene = Scene::new(id, "S", Timeline::new(layers));
        let rate = FrameRate::new(24, 1).expect("rate");
        (
            Document::new(DocumentId::new(), rate, vec![scene], symbols).expect("doc"),
            id,
        )
    }

    fn rect(x: f64, y: f64, w: f64, h: f64, fill: Rgba8) -> Content {
        Content::Shape(Shape::rect(ShapeId::new(), x, y, w, h, fill).expect("rect"))
    }

    fn draw(doc: &Document, scene: SceneId, bg: Rgba8) -> Image {
        draw_sized(doc, scene, bg, 20, 20)
    }

    fn draw_sized(doc: &Document, scene: SceneId, bg: Rgba8, w: u32, h: u32) -> Image {
        let frame = flare_eval::evaluate_frame(doc, scene, 0).expect("eval");
        render(&frame, w, h, bg).expect("render")
    }

    #[test]
    fn filled_rect_is_drawn_over_background() {
        let (doc, id) = scene_with(vec![span(rect(5.0, 5.0, 10.0, 10.0, RED))], vec![]);
        let image = draw(&doc, id, Rgba8::WHITE);
        assert_eq!(image.pixel(10, 10), Some(RED));
        assert_eq!(image.pixel(1, 1), Some(Rgba8::WHITE));
        assert_eq!(image.pixel(20, 0), None);
    }

    #[test]
    fn transparent_background_keeps_straight_alpha() {
        let half = Rgba8::new(200, 100, 50, 128);
        let (doc, id) = scene_with(vec![span(rect(0.0, 0.0, 20.0, 20.0, half))], vec![]);
        let p = draw(&doc, id, Rgba8::TRANSPARENT)
            .pixel(10, 10)
            .expect("pixel");
        assert_eq!(p.a, 128);
        // Unpremultiplied channels stay within rounding of the source color.
        assert!(
            p.r.abs_diff(200) <= 2 && p.g.abs_diff(100) <= 2 && p.b.abs_diff(50) <= 2,
            "{p:?}"
        );
        let (empty, id) = scene_with(vec![span(Content::Empty)], vec![]);
        assert_eq!(
            draw(&empty, id, Rgba8::TRANSPARENT).pixel(3, 3),
            Some(Rgba8::TRANSPARENT)
        );
    }

    #[test]
    fn instance_transforms_move_symbol_shapes() {
        let art = SymbolId::new();
        let art_tl = Timeline::new(vec![
            Layer::new(
                LayerId::new(),
                "L",
                vec![span(rect(0.0, 0.0, 4.0, 4.0, RED))],
            )
            .expect("l"),
        ]);
        let shift =
            Transform2D::new(DAffine2::from_translation(DVec2::new(10.0, 12.0))).expect("finite");
        let instance = Instance::new(InstanceId::new(), art, shift);
        let (doc, id) = scene_with(
            vec![span(Content::Instance(instance))],
            vec![Symbol::new(art, "Art", art_tl)],
        );
        let image = draw(&doc, id, Rgba8::WHITE);
        assert_eq!(image.pixel(11, 13), Some(RED));
        assert_eq!(image.pixel(1, 1), Some(Rgba8::WHITE));
    }

    #[test]
    fn later_layers_in_paint_order_cover_earlier_ones() {
        let blue = Rgba8::opaque(0, 0, 255);
        // First layer is on top, so it paints last.
        let spans = vec![
            span(rect(0.0, 0.0, 20.0, 20.0, RED)),
            span(rect(0.0, 0.0, 20.0, 20.0, blue)),
        ];
        let (doc, id) = scene_with(spans, vec![]);
        assert_eq!(draw(&doc, id, Rgba8::WHITE).pixel(5, 5), Some(RED));
    }

    #[test]
    fn strokes_are_drawn_centered_on_the_path_over_the_fill() {
        use flare_document::Stroke as DocStroke;
        let blue = Rgba8::opaque(0, 0, 255);
        let shape = Shape::rect(ShapeId::new(), 5.0, 5.0, 10.0, 10.0, RED)
            .expect("rect")
            .with_stroke(DocStroke::new(4.0, blue).expect("stroke"));
        let (doc, id) = scene_with(vec![span(Content::Shape(shape))], vec![]);
        let image = draw(&doc, id, Rgba8::WHITE);
        assert_eq!(image.pixel(10, 10), Some(RED), "interior keeps the fill");
        assert_eq!(
            image.pixel(5, 10),
            Some(blue),
            "on the edge: stroke covers the fill"
        );
        assert_eq!(
            image.pixel(4, 10),
            Some(blue),
            "stroke extends half its width outside"
        );
        assert_eq!(
            image.pixel(2, 10),
            Some(Rgba8::WHITE),
            "beyond the stroke is background"
        );
        // An outline-only shape: transparent fill leaves the interior untouched.
        let outline = Shape::rect(ShapeId::new(), 5.0, 5.0, 10.0, 10.0, Rgba8::TRANSPARENT)
            .expect("rect")
            .with_stroke(DocStroke::new(2.0, blue).expect("stroke"));
        let (doc, id) = scene_with(vec![span(Content::Shape(outline))], vec![]);
        let image = draw(&doc, id, Rgba8::WHITE);
        assert_eq!(image.pixel(10, 10), Some(Rgba8::WHITE));
        assert_eq!(image.pixel(5, 10), Some(blue));
    }

    #[test]
    fn hit_testing_finds_the_topmost_shape_through_transforms() {
        let art = SymbolId::new();
        let art_tl = Timeline::new(vec![
            Layer::new(
                LayerId::new(),
                "L",
                vec![span(rect(0.0, 0.0, 10.0, 10.0, RED))],
            )
            .expect("l"),
        ]);
        let shift =
            Transform2D::new(DAffine2::from_translation(DVec2::new(50.0, 50.0))).expect("finite");
        let instance = Instance::new(InstanceId::new(), art, shift);
        let blue = Rgba8::opaque(0, 0, 255);
        // First layer is on top: the blue rectangle covers the left half of the stage.
        let spans = vec![
            span(rect(0.0, 0.0, 55.0, 100.0, blue)),
            span(Content::Instance(instance)),
        ];
        let (doc, id) = scene_with(spans, vec![Symbol::new(art, "Art", art_tl)]);
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        assert_eq!(frame.shapes.len(), 2);
        // (52, 52) is inside both; the top layer wins (paint order: it is drawn last).
        assert_eq!(hit_test(&frame, DVec2::new(52.0, 52.0)), Some(1));
        // (58, 58) is only inside the instance's translated square.
        assert_eq!(hit_test(&frame, DVec2::new(58.0, 58.0)), Some(0));
        assert_eq!(hit_test(&frame, DVec2::new(90.0, 90.0)), None);
        // The instance's local origin square (0..10) was moved away; this point is in the blue rect.
        assert_eq!(hit_test(&frame, DVec2::new(5.0, 80.0)), Some(1));
    }

    #[test]
    fn hit_test_all_lists_every_overlapping_shape_topmost_first() {
        let blue = Rgba8::opaque(0, 0, 255);
        let spans = vec![
            span(rect(0.0, 0.0, 50.0, 50.0, blue)),
            span(rect(0.0, 0.0, 100.0, 100.0, RED)),
        ];
        let (doc, id) = scene_with(spans, vec![]);
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        assert_eq!(hit_test_all(&frame, DVec2::new(10.0, 10.0)), vec![1, 0]);
        assert_eq!(hit_test_all(&frame, DVec2::new(80.0, 80.0)), vec![0]);
        assert!(hit_test_all(&frame, DVec2::new(200.0, 200.0)).is_empty());
    }

    #[test]
    fn transparent_fills_and_singular_transforms_are_not_hittable() {
        let clear = Rgba8::new(1, 2, 3, 0);
        let (doc, id) = scene_with(vec![span(rect(0.0, 0.0, 10.0, 10.0, clear))], vec![]);
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        assert_eq!(hit_test(&frame, DVec2::new(5.0, 5.0)), None);

        let art = SymbolId::new();
        let art_tl = Timeline::new(vec![
            Layer::new(
                LayerId::new(),
                "L",
                vec![span(rect(0.0, 0.0, 10.0, 10.0, RED))],
            )
            .expect("l"),
        ]);
        let flat = Transform2D::new(DAffine2::from_scale(DVec2::new(0.0, 1.0))).expect("finite");
        let instance = Instance::new(InstanceId::new(), art, flat);
        let (doc, id) = scene_with(
            vec![span(Content::Instance(instance))],
            vec![Symbol::new(art, "Art", art_tl)],
        );
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        assert_eq!(hit_test(&frame, DVec2::new(0.0, 5.0)), None);
    }

    #[test]
    fn invalid_sizes_are_rejected() {
        let (doc, id) = scene_with(vec![], vec![]);
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        for (w, h) in [
            (0, 10),
            (10, 0),
            (MAX_DIMENSION + 1, 1),
            (MAX_DIMENSION, MAX_DIMENSION),
        ] {
            assert!(matches!(
                render(&frame, w, h, Rgba8::WHITE),
                Err(RenderError::InvalidSize(..))
            ));
        }
    }

    #[test]
    fn png_round_trips_pixels() {
        let (doc, id) = scene_with(vec![span(rect(5.0, 5.0, 10.0, 10.0, RED))], vec![]);
        let image = draw(&doc, id, Rgba8::WHITE);
        let mut bytes = Vec::new();
        image.write_png(&mut bytes).expect("png");
        let mut reader = png::Decoder::new(std::io::Cursor::new(bytes.as_slice()))
            .read_info()
            .expect("info");
        let mut buf = vec![0; reader.output_buffer_size().expect("size")];
        let info = reader.next_frame(&mut buf).expect("frame");
        assert_eq!((info.width, info.height), (20, 20));
        assert_eq!(&buf[..info.buffer_size()], image.rgba());
    }

    #[test]
    fn fading_scales_alpha_only_and_composition_is_source_over() {
        let (doc, id) = scene_with(vec![span(rect(0.0, 0.0, 20.0, 20.0, RED))], vec![]);
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        let red = render(&frame, 20, 20, Rgba8::TRANSPARENT).expect("render");
        let half = red.faded(0.5);
        assert_eq!(half.pixel(5, 5), Some(Rgba8::new(255, 0, 0, 128)));
        assert_eq!(red.faded(2.0), red, "opacity clamps to 1");
        assert_eq!(red.faded(-1.0).pixel(5, 5).map(|p| p.a), Some(0));

        // 50% red over opaque white: halfway between white and red.
        let mut canvas = render(
            &flare_eval::Frame {
                placed: vec![],
                shapes: vec![],
            },
            20,
            20,
            Rgba8::WHITE,
        )
        .expect("bg");
        canvas.composite(&half).expect("composite");
        let p = canvas.pixel(5, 5).expect("pixel");
        assert_eq!((p.r, p.a), (255, 255));
        assert!(p.g.abs_diff(127) <= 1 && p.b.abs_diff(127) <= 1, "{p:?}");

        // Over a transparent canvas the ghost keeps its own color and alpha.
        let mut clear = render(
            &flare_eval::Frame {
                placed: vec![],
                shapes: vec![],
            },
            20,
            20,
            Rgba8::TRANSPARENT,
        )
        .expect("clear");
        clear.composite(&half).expect("composite");
        assert_eq!(clear.pixel(5, 5), Some(Rgba8::new(255, 0, 0, 128)));
        // Transparent pixels change nothing, and mismatched sizes are refused.
        let before = canvas.clone();
        canvas.composite(&red.faded(0.0)).expect("composite");
        assert_eq!(canvas, before);
        let small = render(
            &flare_eval::Frame {
                placed: vec![],
                shapes: vec![],
            },
            4,
            4,
            Rgba8::WHITE,
        )
        .expect("small");
        assert!(canvas.composite(&small).is_err());
    }

    #[test]
    fn onion_frames_sit_beneath_the_current_frame() {
        let blue = Rgba8::opaque(0, 0, 255);
        // Frame 0 has a red square on the left; frame 1 has a blue square overlapping it.
        let layer = |content: (u32, Content)| {
            Layer::new(
                LayerId::new(),
                "L",
                vec![FrameSpan::new(content.0, NonZeroU32::MIN, content.1)],
            )
            .expect("layer")
        };
        let timeline = Timeline::new(vec![
            layer((1, rect(5.0, 0.0, 10.0, 10.0, blue))),
            layer((0, rect(0.0, 0.0, 10.0, 10.0, RED))),
        ]);
        let id = SceneId::new();
        let scene = Scene::new(id, "S", timeline);
        let rate = FrameRate::new(24, 1).expect("rate");
        let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
        let ghost = flare_eval::evaluate_frame(&doc, id, 0).expect("ghost");
        let current = flare_eval::evaluate_frame(&doc, id, 1).expect("current");

        let image = render_onion(&[(&ghost, 0.5)], &current, 20, 20, Rgba8::WHITE).expect("onion");
        // Ghost only (x 0..5): faint red. Overlap (x 5..10): the current frame's solid blue.
        let faint = image.pixel(2, 5).expect("pixel");
        assert!(faint.r == 255 && faint.g.abs_diff(127) <= 1, "{faint:?}");
        assert_eq!(
            image.pixel(7, 5),
            Some(blue),
            "the current frame covers the ghost"
        );
        assert_eq!(image.pixel(12, 5), Some(blue));
        assert_eq!(image.pixel(18, 5), Some(Rgba8::WHITE));

        // Without ghosts it is exactly the normal render.
        let plain = render(&current, 20, 20, Rgba8::WHITE).expect("plain");
        assert_eq!(
            render_onion(&[], &current, 20, 20, Rgba8::WHITE).expect("onion"),
            plain
        );
    }

    #[test]
    fn strokes_are_hittable_where_they_are_drawn_including_lines_and_outlines() {
        use flare_document::Stroke as DocStroke;
        let ink = Rgba8::opaque(0, 0, 0);
        let line = Shape::line(
            ShapeId::new(),
            (10.0, 10.0),
            (90.0, 10.0),
            DocStroke::new(6.0, ink).expect("stroke"),
        )
        .expect("line");
        let outline = Shape::rect(ShapeId::new(), 10.0, 40.0, 40.0, 40.0, Rgba8::TRANSPARENT)
            .expect("rect")
            .with_stroke(DocStroke::new(4.0, ink).expect("stroke"));
        let (doc, id) = scene_with(
            vec![span(Content::Shape(line)), span(Content::Shape(outline))],
            vec![],
        );
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        let at = |x: f64, y: f64| hit_test_all(&frame, DVec2::new(x, y));
        // Paint order is bottom layer first, so the outline is shape 0 and the line shape 1.
        // On the line (within half the width) and just past it, including the round cap.
        assert_eq!(at(50.0, 10.0), vec![1]);
        assert_eq!(at(50.0, 12.9), vec![1]);
        assert!(at(50.0, 13.5).is_empty(), "outside the 3px half-width");
        assert_eq!(
            at(92.0, 10.0),
            vec![1],
            "the round cap extends past the end"
        );
        assert!(at(95.0, 10.0).is_empty());
        // An outline-only rectangle: on its edge yes, in its empty middle no.
        assert_eq!(at(10.0, 60.0), vec![0]);
        assert_eq!(at(31.0, 41.0), vec![0]);
        assert!(
            at(30.0, 60.0).is_empty(),
            "transparent fill has nothing to click inside"
        );
    }

    #[test]
    fn a_stroked_filled_shape_is_hit_on_its_fill_and_just_outside_on_the_stroke() {
        use flare_document::Stroke as DocStroke;
        let both = Shape::rect(ShapeId::new(), 20.0, 20.0, 20.0, 20.0, RED)
            .expect("rect")
            .with_stroke(DocStroke::new(8.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
        let (doc, id) = scene_with(vec![span(Content::Shape(both))], vec![]);
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        let at = |x: f64, y: f64| hit_test_all(&frame, DVec2::new(x, y));
        assert_eq!(at(30.0, 30.0), vec![0], "inside the fill");
        assert_eq!(
            at(18.0, 30.0),
            vec![0],
            "on the stroke outside the fill edge"
        );
        assert!(at(14.0, 30.0).is_empty(), "beyond the stroke");
    }

    #[test]
    fn stroke_width_follows_the_instances_scale() {
        use flare_document::Stroke as DocStroke;
        let art = SymbolId::new();
        let line = Shape::line(
            ShapeId::new(),
            (0.0, 0.0),
            (20.0, 0.0),
            DocStroke::new(2.0, Rgba8::opaque(0, 0, 0)).expect("stroke"),
        )
        .expect("line");
        let art_tl = Timeline::new(vec![
            Layer::new(LayerId::new(), "L", vec![span(Content::Shape(line))]).expect("l"),
        ]);
        let scale = Transform2D::new(
            DAffine2::from_scale(DVec2::new(3.0, 3.0))
                * DAffine2::from_translation(DVec2::new(10.0, 10.0)),
        )
        .expect("finite");
        let instance = Instance::new(InstanceId::new(), art, scale);
        let (doc, id) = scene_with(
            vec![span(Content::Instance(instance))],
            vec![Symbol::new(art, "Art", art_tl)],
        );
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        // The 2-unit stroke is 6px wide at 3x, so 2.9px from the line's centre is still on it.
        assert_eq!(hit_test_all(&frame, DVec2::new(60.0, 32.9)), vec![0]);
        assert!(hit_test_all(&frame, DVec2::new(60.0, 33.5)).is_empty());
    }

    fn gradient_rect(w: f64, h: f64) -> Shape {
        use flare_core::DocumentPoint;
        let end = DocumentPoint::new(w, 0.0).expect("finite");
        let start = DocumentPoint::new(0.0, 0.0).expect("finite");
        let gradient = flare_document::Gradient::linear(
            start,
            end,
            Rgba8::opaque(255, 0, 0),
            Rgba8::opaque(0, 0, 255),
        )
        .expect("gradient");
        Shape::rect(ShapeId::new(), 0.0, 0.0, w, h, Rgba8::WHITE)
            .expect("rect")
            .with_gradient(gradient)
    }

    #[test]
    fn a_linear_gradient_blends_between_its_two_points() {
        let (doc, id) = scene_with(
            vec![span(Content::Shape(gradient_rect(100.0, 20.0)))],
            vec![],
        );
        let image = draw_sized(&doc, id, Rgba8::WHITE, 100, 20);
        let at = |x| image.pixel(x, 10).expect("pixel");
        assert!(at(2).r > 230 && at(2).b < 25, "red end: {:?}", at(2));
        assert!(at(97).b > 230 && at(97).r < 25, "blue end: {:?}", at(97));
        let mid = at(50);
        assert!(
            mid.r.abs_diff(127) < 20 && mid.b.abs_diff(127) < 20,
            "middle: {mid:?}"
        );
        assert!(at(30).r > at(70).r, "red falls off left to right");
    }

    #[test]
    fn the_gradient_follows_the_instance_transform() {
        // The same 100-wide gradient, scaled to half size by the instance: the blue end now
        // sits at x = 50, not 100.
        let art = SymbolId::new();
        let art_tl = Timeline::new(vec![
            Layer::new(
                LayerId::new(),
                "L",
                vec![span(Content::Shape(gradient_rect(100.0, 20.0)))],
            )
            .expect("l"),
        ]);
        let half = Transform2D::new(DAffine2::from_scale(DVec2::splat(0.5))).expect("finite");
        let instance = Instance::new(InstanceId::new(), art, half);
        let (doc, id) = scene_with(
            vec![span(Content::Instance(instance))],
            vec![Symbol::new(art, "Art", art_tl)],
        );
        let image = draw_sized(&doc, id, Rgba8::WHITE, 100, 20);
        let end = image.pixel(48, 5).expect("pixel");
        assert!(end.b > 220 && end.r < 35, "blue at the scaled end: {end:?}");
        assert_eq!(
            image.pixel(70, 5),
            Some(Rgba8::WHITE),
            "nothing beyond the shape"
        );
    }

    #[test]
    fn a_gradient_over_a_clear_fill_is_hittable_and_a_clear_one_is_not() {
        use flare_core::DocumentPoint;
        let visible = gradient_rect(40.0, 40.0);
        let p = |x| DocumentPoint::new(x, 0.0).expect("finite");
        let clear = Shape::rect(ShapeId::new(), 0.0, 0.0, 40.0, 40.0, Rgba8::WHITE)
            .expect("rect")
            .with_gradient(
                flare_document::Gradient::linear(
                    p(0.0),
                    p(40.0),
                    Rgba8::TRANSPARENT,
                    Rgba8::TRANSPARENT,
                )
                .expect("gradient"),
            );
        for (shape, hit) in [(visible, true), (clear, false)] {
            let (doc, id) = scene_with(vec![span(Content::Shape(shape))], vec![]);
            let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("frame");
            assert_eq!(
                hit_test(&frame, glamx::DVec2::new(20.0, 20.0)).is_some(),
                hit
            );
        }
    }

    #[test]
    fn a_viewport_draws_the_chosen_part_of_the_stage_at_the_chosen_resolution() {
        // Stage 0..100: a red square at 40..60 and a blue one at 80..100.
        let blue = Rgba8::opaque(0, 0, 255);
        let (doc, id) = scene_with(
            vec![
                span(rect(40.0, 40.0, 20.0, 20.0, RED)),
                span(rect(80.0, 80.0, 20.0, 20.0, blue)),
            ],
            vec![],
        );
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");

        // The right-bottom quarter (50..100) shown at 2 pixels per unit: 100 x 100 pixels.
        let view = Viewport {
            x: 50.0,
            y: 50.0,
            width: 50.0,
            height: 50.0,
        };
        let image = render_viewport(&frame, &view, 100, 100, Rgba8::WHITE).expect("render");
        assert_eq!((image.width(), image.height()), (100, 100));
        assert_eq!(
            image.pixel(5, 5),
            Some(RED),
            "stage (52, 52) is inside the red square"
        );
        assert_eq!(
            image.pixel(25, 25),
            Some(Rgba8::WHITE),
            "stage (62, 62) is between the squares"
        );
        assert_eq!(image.pixel(80, 80), Some(blue), "stage (90, 90)");
        assert_eq!(
            image.pixel(19, 5),
            Some(RED),
            "the red square ends at stage 60, which is pixel 20"
        );
        assert_eq!(image.pixel(20, 5), Some(Rgba8::WHITE));

        // The whole-stage viewport is exactly what `render` draws.
        let full = render_viewport(&frame, &Viewport::full(100, 100), 100, 100, Rgba8::WHITE)
            .expect("full");
        assert_eq!(
            full,
            render(&frame, 100, 100, Rgba8::WHITE).expect("render")
        );
    }

    #[test]
    fn viewports_work_with_strokes_and_gradients_and_reject_bad_requests() {
        let shape = gradient_rect(100.0, 20.0);
        let (doc, id) = scene_with(vec![span(Content::Shape(shape))], vec![]);
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        // Zoom into the right half: the blue end of the gradient fills the view.
        let right = Viewport {
            x: 90.0,
            y: 0.0,
            width: 10.0,
            height: 20.0,
        };
        let image = render_viewport(&frame, &right, 50, 100, Rgba8::WHITE).expect("render");
        let end = image.pixel(45, 50).expect("pixel");
        assert!(end.b > 220 && end.r < 40, "{end:?}");
        for bad in [
            Viewport {
                x: f64::NAN,
                ..right
            },
            Viewport {
                width: 0.0,
                ..right
            },
            Viewport {
                height: -1.0,
                ..right
            },
            Viewport {
                y: f64::INFINITY,
                ..right
            },
        ] {
            assert!(
                render_viewport(&frame, &bad, 10, 10, Rgba8::WHITE).is_err(),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn premultiplied_output_matches_the_straight_image_within_rounding() {
        let half = Rgba8::new(200, 100, 50, 128);
        let (doc, id) = scene_with(vec![span(rect(2.0, 2.0, 10.0, 10.0, half))], vec![]);
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        let view = Viewport::full(20, 20);
        let straight =
            render_viewport(&frame, &view, 20, 20, Rgba8::TRANSPARENT).expect("straight");
        let premul = render_viewport_premultiplied(&frame, &view, 20, 20, Rgba8::TRANSPARENT)
            .expect("premultiplied");
        assert_eq!((premul.width(), premul.height()), (20, 20));
        let s = straight.pixel(5, 5).expect("pixel");
        let i = (5 * 20 + 5) * 4;
        let p = &premul.rgba()[i..i + 4];
        assert_eq!(p[3], s.a);
        for (premultiplied, straight_channel) in p[..3].iter().zip([s.r, s.g, s.b]) {
            let expected = (u32::from(straight_channel) * u32::from(s.a) + 127) / 255;
            assert!(
                u32::from(*premultiplied).abs_diff(expected) <= 1,
                "{p:?} vs {s:?}"
            );
        }
        // Outside the square both are fully transparent black.
        assert_eq!(&premul.rgba()[..4], &[0, 0, 0, 0]);
        // An opaque render is the same bytes either way.
        let opaque_a = render_viewport(&frame, &view, 20, 20, Rgba8::WHITE).expect("opaque");
        let opaque_b = render_viewport_premultiplied(&frame, &view, 20, 20, Rgba8::WHITE)
            .expect("opaque premultiplied");
        assert_eq!(opaque_a.rgba(), opaque_b.rgba());
    }

    #[test]
    fn small_frames_and_small_machines_stay_on_one_thread() {
        assert_eq!(render_threads(0, 16), 0);
        assert_eq!(
            render_threads(PARALLEL_MIN_SHAPES - 1, 16),
            0,
            "too little work"
        );
        assert_eq!(render_threads(PARALLEL_MIN_SHAPES, 16), 4, "capped");
        assert_eq!(render_threads(100_000, 64), MAX_RENDER_THREADS);
        assert_eq!(
            render_threads(5_000, 4),
            3,
            "leaves one core for the caller"
        );
        assert_eq!(
            render_threads(5_000, 2),
            0,
            "two cores are not worth splitting"
        );
        assert_eq!(render_threads(5_000, 1), 0);
        assert_eq!(render_threads(5_000, 0), 0);
    }

    #[test]
    fn threaded_rendering_matches_drawing_the_frame_in_pieces() {
        // Enough shapes to take the threaded path, overlapping and translucent so that the
        // order in which layers are blended matters.
        let spans: Vec<FrameSpan> = (0..PARALLEL_MIN_SHAPES + 50)
            .map(|i| {
                let x = (i % 30) as f64 * 3.0;
                let y = (i / 30) as f64 * 3.0;
                span(rect(
                    x,
                    y,
                    14.0,
                    14.0,
                    Rgba8::new((i % 250) as u8, 90, 160, 140),
                ))
            })
            .collect();
        let (doc, id) = scene_with(spans, vec![]);
        let frame = flare_eval::evaluate_frame(&doc, id, 0).expect("eval");
        let threaded = render(&frame, 96, 96, Rgba8::WHITE).expect("threaded");
        let alone = {
            let mut one = frame.clone();
            // The same frame drawn in pieces too small to thread, then composited in order.
            one.shapes.truncate(PARALLEL_MIN_SHAPES - 1);
            let first = render(&one, 96, 96, Rgba8::WHITE).expect("first");
            let rest_frame = flare_eval::Frame {
                shapes: frame.shapes[PARALLEL_MIN_SHAPES - 1..].to_vec(),
                ..frame.clone()
            };
            let mut image = first;
            image
                .composite(&render(&rest_frame, 96, 96, Rgba8::TRANSPARENT).expect("rest"))
                .expect("composite");
            image
        };
        let differing = threaded
            .rgba()
            .iter()
            .zip(alone.rgba())
            .filter(|(a, b)| a.abs_diff(**b) > 2)
            .count();
        assert_eq!(differing, 0, "threads must not change the picture");
    }
}
