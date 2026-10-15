//! Renders every stroke style to a PNG: `cargo run -p ac-render --example stroke_gallery out.png`.
use ac_core::geom::{pt, Path, Seg};
use ac_core::*;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "strokes.png".into());
    let mut d = Document { width: 900.0, height: 700.0, ..Default::default() };
    let mut els = vec![];
    for (i, dash) in Dash::ALL.into_iter().enumerate() {
        let (col, row) = (i / 10, i % 10);
        let (x, y) = (40.0 + col as f32 * 440.0, 40.0 + row as f32 * 68.0);
        let path = Path { segs: vec![Seg::M(pt(x, y)), Seg::C(pt(x + 120.0, y - 40.0), pt(x + 240.0, y + 40.0), pt(x + 360.0, y))] };
        let mut sh = Shape::new(path, None, Some(Stroke { color: Color::rgb(30, 90, 200), width: 12.0 }));
        sh.stroke_style.dash = dash;
        els.push(Element::shape(dash.name(), sh));
    }
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(els));
    ac_render::export_png(&d, 0, 1.0, std::path::Path::new(&out)).unwrap();
}
