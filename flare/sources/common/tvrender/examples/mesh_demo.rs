//! Asset Warp on a bitmap and a symbol: `cargo run -p ac-render --example mesh_demo out.png`.
use ac_core::deform::MeshWarp;
use ac_core::geom::{pt, Path};
use ac_core::*;

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "mesh.png".into());
    let mut d = Document { width: 600.0, height: 300.0, ..Default::default() };
    // a checkerboard bitmap
    let (w, h) = (160u32, 120u32);
    let rgba: Vec<u8> = (0..h).flat_map(|y| (0..w).flat_map(move |x| if ((x / 20) + (y / 20)) % 2 == 0 { [40, 90, 200, 255] } else { [250, 210, 60, 255] })).collect();
    d.bitmaps.push(bitmap::BitmapAsset::from_rgba(1, "checks", w, h, &rgba).unwrap());
    let mut b = Element::bitmap("", 1, Matrix::translate(40.0, 60.0));
    b.mesh = Some(MeshWarp { pins: vec![(pt(0.0, 0.0), pt(0.0, 0.0)), (pt(0.0, 120.0), pt(0.0, 120.0)), (pt(160.0, 0.0), pt(160.0, 60.0)), (pt(160.0, 120.0), pt(190.0, 160.0))] });
    // a symbol with vector art
    let mut tl = Timeline::new("Star");
    tl.layers[0].keyframes.insert(0, Keyframe::with(vec![Element::shape("s", Shape::new(Path::polystar(0.0, 0.0, 70.0, 70.0, 5, true, 0.45), Some(Color::rgb(220, 60, 60)), Some(Stroke { color: Color::rgb(30, 30, 30), width: 3.0 })))]));
    let sym = d.add_symbol("Star", SymbolKind::Graphic, tl);
    let mut inst = Element::instance("", sym, Matrix::translate(430.0, 150.0));
    inst.mesh = Some(MeshWarp { pins: vec![(pt(0.0, 70.0), pt(0.0, 70.0)), (pt(-60.0, 0.0), pt(-60.0, 0.0)), (pt(0.0, -70.0), pt(60.0, -90.0))] });
    d.scene.layers[0].keyframes.insert(0, Keyframe::with(vec![b, inst]));
    ac_render::export_png(&d, 0, 1.5, std::path::Path::new(&out)).unwrap();
}
