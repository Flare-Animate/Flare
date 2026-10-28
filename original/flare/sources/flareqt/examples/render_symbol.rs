//! Debug helper: render one library symbol of a .fla (or any document) to PNG,
//! fitted to its bounds.  cargo run --release -p ac-cli --example render_symbol -- file.fla "name part" [frame] out.png
use ac_core::model::{evaluate, EvalCtx};
use ac_core::Matrix;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let doc = ac_xfl::open(std::path::Path::new(&a[1])).expect("open").doc;
    let want = a[2].to_lowercase();
    let frame: u32 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(1);
    let out = a.last().unwrap();
    let sym = doc.library.iter().find(|s| s.name.to_lowercase() == want).or_else(|| doc.library.iter().find(|s| s.name.to_lowercase().contains(&want))).expect("no such symbol");
    eprintln!("symbol {} ({} layers)", sym.name, sym.timeline.layers.len());
    let ctx = EvalCtx::new(&doc, frame - 1);
    let mut items = vec![];
    evaluate(&ctx, &sym.timeline, frame - 1, &Matrix::IDENTITY, 1.0, 1, &mut items);
    let (lo, hi) = items.iter().filter_map(|it| it.path.transformed(&it.matrix).bounds()).fold((ac_core::pt(f32::MAX, f32::MAX), ac_core::pt(f32::MIN, f32::MIN)), |(l, h), (a, b)| (ac_core::pt(l.x.min(a.x), l.y.min(a.y)), ac_core::pt(h.x.max(b.x), h.y.max(b.y))));
    eprintln!("bounds {lo:?} {hi:?}, {} items", items.len());
    if let Some(it) = items.first() {
        eprintln!("first item matrix {:?}", it.matrix);
    }
    if std::env::var_os("SHOW_BIG").is_some() {
        for (i, it) in items.iter().enumerate() {
            if let Some((a, b)) = it.path.transformed(&it.matrix).bounds() {
                if b.x - a.x > 20000.0 || b.y - a.y > 20000.0 {
                    eprintln!("item {i}: {a:?}..{b:?} fill {:?} stroke {:?} image {} matrix {:?} segs {}", it.fill, it.stroke.map(|s| s.width), it.image.is_some(), it.matrix, it.path.segs.len());
                }
            }
        }
    }
    let s = (800.0 / (hi.x - lo.x).max(hi.y - lo.y).max(1.0)).min(8.0);
    let (w, h) = (((hi.x - lo.x) * s).ceil().max(1.0) as u32, ((hi.y - lo.y) * s).ceil().max(1.0) as u32);
    let mut pix = tiny_skia::Pixmap::new(w, h).unwrap();
    pix.fill(tiny_skia::Color::from_rgba8(200, 200, 200, 255));
    let view = Matrix::scale(s, s).concat(&Matrix::translate(-lo.x, -lo.y));
    ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view, ..Default::default() });
    pix.save_png(out).unwrap();
}
