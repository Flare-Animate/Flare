//! Renderer comparison harness. Usage:
//!   renderer-bench render <backend> <scene> <WxH> [--out f.png] [--repeat N]
//!   renderer-bench diff <a.png> <b.png>
//! Without --out nothing is encoded, so hyperfine measures scene build + raster only.

mod backends;
mod scene;

use std::fs::File;
use std::io::BufWriter;
use std::time::Instant;

fn write_png(path: &str, rgba: &[u8], w: u32, h: u32) {
    let mut encoder = png::Encoder::new(BufWriter::new(File::create(path).expect("create")), w, h);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().expect("header").write_image_data(rgba).expect("data");
}

fn read_png(path: &str) -> (Vec<u8>, u32, u32) {
    let decoder = png::Decoder::new(std::io::BufReader::new(File::open(path).expect("open")));
    let mut reader = decoder.read_info().expect("info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    buf.truncate(info.buffer_size());
    (buf, info.width, info.height)
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * p).round() as usize]
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("render") => {
            let (backend, scene_name, size) = (&args[1], &args[2], &args[3]);
            let (w, h) = size.split_once('x').expect("WxH");
            let (w, h): (u32, u32) = (w.parse().expect("w"), h.parse().expect("h"));
            let opt = |flag: &str| args.iter().position(|a| a == flag).map(|i| args[i + 1].clone());
            let repeat: usize = opt("--repeat").map_or(1, |v| v.parse().expect("repeat"));
            let items = scene::build(scene_name, w as f32, h as f32).expect("scene");
            let mut times = Vec::new();
            let mut last = Vec::new();
            for _ in 0..repeat {
                let start = Instant::now();
                last = backends::render(backend, &items, w, h);
                times.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            if repeat > 1 {
                times.sort_by(f64::total_cmp);
                println!(
                    "{backend} {scene_name} {size}: n={repeat} p50={:.2}ms p95={:.2}ms min={:.2}ms",
                    percentile(&times, 0.5),
                    percentile(&times, 0.95),
                    times[0]
                );
            }
            if let Some(out) = opt("--out") {
                write_png(&out, &last, w, h);
            }
            // Keep the work observable so it cannot be optimized away.
            std::hint::black_box(&last);
        }
        Some("diff") => {
            let (a, aw, ah) = read_png(&args[1]);
            let (b, bw, bh) = read_png(&args[2]);
            assert_eq!((aw, ah), (bw, bh), "size mismatch");
            let (mut sum, mut max, mut over8, mut over32) = (0_u64, 0_u8, 0_u64, 0_u64);
            for (pa, pb) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
                let d = (0..3).map(|i| pa[i].abs_diff(pb[i])).max().unwrap_or(0);
                sum += u64::from(d);
                max = max.max(d);
                over8 += u64::from(d > 8);
                over32 += u64::from(d > 32);
            }
            let pixels = u64::from(aw) * u64::from(ah);
            println!(
                "mean_abs={:.4} max={} px>8={} ({:.4}%) px>32={} ({:.4}%)",
                sum as f64 / pixels as f64,
                max,
                over8,
                over8 as f64 * 100.0 / pixels as f64,
                over32,
                over32 as f64 * 100.0 / pixels as f64
            );
        }
        _ => eprintln!("usage: render|diff (see source header)"),
    }
}
