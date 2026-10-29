//! One function per candidate. Each returns straight RGBA8 over an opaque white background.

use crate::scene::{Item, Seg};


pub fn render(backend: &str, items: &[Item], w: u32, h: u32) -> Vec<u8> {
    match backend {
        "tiny-skia" => tiny(items, w, h),
        "vello-cpu" => vello(items, w, h),
        "agg" => agg(items, w, h),
        other => panic!("unknown backend {other}"),
    }
}

fn tiny(items: &[Item], w: u32, h: u32) -> Vec<u8> {
    use tiny_skia::{
        Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform,
    };
    let mut pixmap = Pixmap::new(w, h).expect("pixmap");
    pixmap.fill(Color::WHITE);
    for item in items {
        let mut pb = PathBuilder::new();
        for seg in &item.segs {
            match *seg {
                Seg::Move(x, y) => pb.move_to(x, y),
                Seg::Line(x, y) => pb.line_to(x, y),
                Seg::Cubic(a, b, c, d, e, f) => pb.cubic_to(a, b, c, d, e, f),
                Seg::Close => pb.close(),
            }
        }
        let Some(path) = pb.finish() else { continue };
        let mut paint = Paint::default();
        paint.set_color_rgba8(item.rgba[0], item.rgba[1], item.rgba[2], item.rgba[3]);
        paint.anti_alias = true;
        match item.stroke {
            None => {
                pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None)
            }
            Some(width) => {
                let stroke = Stroke {
                    width,
                    line_cap: LineCap::Round,
                    line_join: LineJoin::Round,
                    ..Stroke::default()
                };
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        }
    }
    // Opaque background, so premultiplied == straight.
    pixmap.data().to_vec()
}

fn vello(items: &[Item], w: u32, h: u32) -> Vec<u8> {
    use vello_cpu::color::{AlphaColor, Srgb};
    use vello_cpu::kurbo::{BezPath, Cap, Join, Rect, Stroke};
    use vello_cpu::{Pixmap, RenderContext, Resources};

    let (w16, h16) = (w as u16, h as u16);
    let mut ctx = RenderContext::new(w16, h16);
    let mut resources = Resources::new();
    ctx.set_paint(AlphaColor::<Srgb>::from_rgba8(255, 255, 255, 255));
    ctx.fill_rect(&Rect::new(0.0, 0.0, f64::from(w), f64::from(h)));
    for item in items {
        let mut path = BezPath::new();
        for seg in &item.segs {
            match *seg {
                Seg::Move(x, y) => path.move_to((f64::from(x), f64::from(y))),
                Seg::Line(x, y) => path.line_to((f64::from(x), f64::from(y))),
                Seg::Cubic(a, b, c, d, e, f) => path.curve_to(
                    (f64::from(a), f64::from(b)),
                    (f64::from(c), f64::from(d)),
                    (f64::from(e), f64::from(f)),
                ),
                Seg::Close => path.close_path(),
            }
        }
        let [r, g, b, a] = item.rgba;
        ctx.set_paint(AlphaColor::<Srgb>::from_rgba8(r, g, b, a));
        match item.stroke {
            None => ctx.fill_path(&path),
            Some(width) => {
                ctx.set_stroke(Stroke::new(f64::from(width)).with_caps(Cap::Round).with_join(Join::Round));
                ctx.stroke_path(&path);
            }
        }
    }
    ctx.flush();
    let mut pixmap = Pixmap::new(w16, h16);
    ctx.render(&mut pixmap, &mut resources);
    pixmap.data_as_u8_slice().to_vec()
}

fn agg(items: &[Item], w: u32, h: u32) -> Vec<u8> {
    use agg_rust::basics::PATH_FLAGS_CLOSE;
    use agg_rust::color::Rgba8;
    use agg_rust::conv_curve::ConvCurve;
    use agg_rust::conv_stroke::ConvStroke;
    use agg_rust::math_stroke::{LineCap, LineJoin};
    use agg_rust::path_storage::PathStorage;
    use agg_rust::pixfmt_rgba::PixfmtRgba32;
    use agg_rust::rasterizer_scanline_aa::RasterizerScanlineAa;
    use agg_rust::renderer_base::RendererBase;
    use agg_rust::renderer_scanline::render_scanlines_aa_solid;
    use agg_rust::rendering_buffer::RowAccessor;
    use agg_rust::scanline_u::ScanlineU8;

    let mut buf = vec![0_u8; (w * h * 4) as usize];
    // SAFETY: `buf` outlives `rbuf` and is exactly height * stride bytes.
    let mut rbuf = unsafe { RowAccessor::new_with_buf(buf.as_mut_ptr(), w, h, (w * 4) as i32) };
    {
        let pixfmt = PixfmtRgba32::new(&mut rbuf);
        let mut base = RendererBase::new(pixfmt);
        base.clear(&Rgba8::new_opaque(255, 255, 255));
        let mut ras = RasterizerScanlineAa::new();
        let mut sl = ScanlineU8::new();
        for item in items {
            let mut path = PathStorage::new();
            for seg in &item.segs {
                match *seg {
                    Seg::Move(x, y) => path.move_to(f64::from(x), f64::from(y)),
                    Seg::Line(x, y) => path.line_to(f64::from(x), f64::from(y)),
                    Seg::Cubic(a, b, c, d, e, f) => path.curve4(
                        f64::from(a), f64::from(b), f64::from(c), f64::from(d), f64::from(e), f64::from(f),
                    ),
                    Seg::Close => path.close_polygon(PATH_FLAGS_CLOSE),
                }
            }
            let [r, g, b, a] = item.rgba;
            let color = Rgba8::new(u32::from(r), u32::from(g), u32::from(b), u32::from(a));
            ras.reset();
            match item.stroke {
                None => {
                    let mut curve = ConvCurve::new(path);
                    ras.add_path(&mut curve, 0);
                }
                Some(width) => {
                    let curve = ConvCurve::new(path);
                    let mut stroke = ConvStroke::new(curve);
                    stroke.set_width(f64::from(width));
                    stroke.set_line_cap(LineCap::Round);
                    stroke.set_line_join(LineJoin::Round);
                    ras.add_path(&mut stroke, 0);
                }
            }
            render_scanlines_aa_solid(&mut ras, &mut sl, &mut base, &color);
        }
    }
    buf
}
