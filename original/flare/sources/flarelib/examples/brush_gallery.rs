//! Paints every bundled MyPaint brush to a PNG:
//! `cargo run -p ac-core --example brush_gallery out.png`.
use ac_core::bitmap::{BitmapAsset, Image};
use ac_core::model::Color;
use ac_core::paint::{PaintOpts, PaintStroke, PRESETS};

fn main() {
    let out = std::env::args().nth(1).unwrap_or_else(|| "brushes.png".into());
    let (w, row) = (900u32, 40u32);
    let h = row * PRESETS.len() as u32 + 20;
    // a light background so erasers and smudges show
    let mut img = Image { width: w, height: h, rgba: vec![0; (w * h * 4) as usize], smoothing: true };
    for (i, px) in img.rgba.chunks_mut(4).enumerate() {
        let x = (i as u32) % w;
        if x > 600 {
            px.copy_from_slice(&[230, 200, 120, 255]);
        }
    }
    for (k, p) in PRESETS.iter().enumerate() {
        let y = 20.0 + k as f32 * row as f32;
        let o = PaintOpts { color: Color::rgb(30, 60, 160), radius: Some(p.base_radius().clamp(2.0, 12.0)), ..Default::default() };
        let mut s = PaintStroke::new(&img, p, &o).unwrap();
        for i in 0..=400 {
            let t = i as f32 / 400.0;
            s.to(20.0 + t * 860.0, y + (t * 12.0).sin() * 8.0, Some(0.3 + 0.6 * (t * std::f32::consts::PI).sin()), 0.008);
        }
        s.write_into(&mut img);
        println!("{:2} {}", k + 1, p.id);
    }
    let a = BitmapAsset::from_rgba(0, "g", w, h, &ac_core::paint::unpremultiply(&img)).unwrap();
    std::fs::write(out, a.png).unwrap();
}
