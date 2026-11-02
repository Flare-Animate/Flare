//! Draws line art, traces it with Trace Bitmap ▸ Centerline and renders both:
//! `cargo run -p ac-render --example centerline_demo out.png`.
use ac_core::geom::{pt, Path, Seg};
use ac_core::*;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "centerline.png".into());
    // line art: a face-ish doodle with lines of several widths
    let mut art = Document { width: 300.0, height: 220.0, ..Default::default() };
    let mut els = vec![];
    let mut add = |p: Path, w: f32| els.push(Element::shape("l", Shape::new(p, None, Some(Stroke { color: Color::rgb(20, 20, 30), width: w }))));
    add(Path::ellipse(150.0, 110.0, 90.0, 80.0), 6.0);
    add(Path::ellipse(115.0, 90.0, 14.0, 10.0), 3.0);
    add(Path::ellipse(185.0, 90.0, 14.0, 10.0), 3.0);
    add(Path { segs: vec![Seg::M(pt(100.0, 140.0)), Seg::C(pt(130.0, 180.0), pt(170.0, 180.0), pt(200.0, 140.0))] }, 4.0);
    add(Path { segs: vec![Seg::M(pt(150.0, 95.0)), Seg::L(pt(140.0, 125.0)), Seg::L(pt(155.0, 125.0))] }, 2.0);
    add(Path { segs: vec![Seg::M(pt(20.0, 20.0)), Seg::L(pt(280.0, 200.0))] }, 1.5);
    art.scene.layers[0].keyframes.insert(0, Keyframe::with(els));
    let pix = ac_render::render_frame_opts(&art, 0, 1.0, false);
    // trace it
    let rgba = pix.data().to_vec();
    let m = trace::trace(&rgba, pix.width(), pix.height(), &trace::TraceOpts { mode: trace::TraceMode::Centerline, curve_fit: trace::CurveFit::Smooth, ..Default::default() });
    println!("{} lines", m.strokes.len());
    // side by side: original on the left, traced lines (in red) on the right
    let mut both = Document { width: 600.0, height: 220.0, ..Default::default() };
    let mut els = vec![];
    let id = {
        let a = bitmap::BitmapAsset::from_rgba(1, "art", pix.width(), pix.height(), &ac_render::unpremultiplied(&pix)).unwrap();
        both.bitmaps.push(a);
        1
    };
    els.push(Element::bitmap("", id, Matrix::IDENTITY));
    let mut traced = m.clone();
    for s in &mut traced.strokes {
        s.stroke.color = Color::rgb(200, 30, 30);
    }
    let mut el = Element::shape("t", Shape::default());
    el.kind = ElementKind::Canvas(traced);
    el.matrix = Matrix::translate(300.0, 0.0);
    els.push(el);
    both.scene.layers[0].keyframes.insert(0, Keyframe::with(els));
    ac_render::export_png(&both, 0, 2.0, std::path::Path::new(&out)).unwrap();
}
