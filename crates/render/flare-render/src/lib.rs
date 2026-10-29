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
use vello_cpu::kurbo::{Affine, BezPath, Point, Rect, Shape as _};
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

/// Index into `frame.shapes` of the topmost shape whose fill contains `point` (scene
/// coordinates), using the same non-zero fill rule as rendering. Fully transparent
/// fills and singular transforms are not hittable.
pub fn hit_test(frame: &Frame<'_>, point: glamx::DVec2) -> Option<usize> {
    frame
        .shapes
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, drawn)| {
            if drawn.shape.fill().a == 0 || drawn.transform.matrix2.determinant() == 0.0 {
                return None;
            }
            let local = drawn.transform.inverse().transform_point2(point);
            let inside = bez_path(drawn.shape.path()).contains(Point::new(local.x, local.y));
            inside.then_some(index)
        })
}

/// Converts premultiplied to straight alpha with rounding.
fn unpremultiply(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for px in data.as_chunks::<4>().0 {
        let a = u32::from(px[3]);
        if a == 0 || a == 255 {
            out.extend_from_slice(&px[..]);
        } else {
            for &c in &px[..3] {
                out.push(((u32::from(c) * 255 + a / 2) / a).min(255) as u8);
            }
            out.push(px[3]);
        }
    }
    out
}

/// Rasterizes `frame` at `width` x `height` over `background` (use
/// [`Rgba8::TRANSPARENT`] for no background).
pub fn render(
    frame: &Frame<'_>,
    width: u32,
    height: u32,
    background: Rgba8,
) -> Result<Image, RenderError> {
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
    let mut ctx = RenderContext::new(w16, h16);
    if background.a > 0 {
        ctx.set_paint(color(background));
        ctx.fill_rect(&Rect::new(0.0, 0.0, f64::from(width), f64::from(height)));
    }
    for drawn in &frame.shapes {
        let [a, b, c, d, e, f] = drawn.transform.to_cols_array();
        ctx.set_transform(Affine::new([a, b, c, d, e, f]));
        ctx.set_paint(color(drawn.shape.fill()));
        ctx.fill_path(&bez_path(drawn.shape.path()));
    }
    ctx.flush();
    let mut pixmap = Pixmap::new(w16, h16);
    ctx.render(&mut pixmap, &mut Resources::new());
    Ok(Image {
        width,
        height,
        rgba: unpremultiply(pixmap.data_as_u8_slice()),
    })
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
        let frame = flare_eval::evaluate_frame(doc, scene, 0).expect("eval");
        render(&frame, 20, 20, bg).expect("render")
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
}
