//! What each kind of node costs on a 1280x720 picture (a stage panel's worth of pixels), and a
//! shadow with snow at 1920x1080. `cargo bench -p flare-fx`.
#![allow(clippy::expect_used)]

use criterion::{Criterion, criterion_group, criterion_main};
use flare_fx::{EffectGraph, EffectKind, Pixels, Space, run};

fn picture(w: u32, h: u32) -> Pixels {
    // A disc in the middle, so edges and empty space are both in the picture.
    let (cx, cy, r) = (w as f64 / 2.0, h as f64 / 2.0, h as f64 / 3.0);
    let mut data = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f64 - cx).powi(2) + (y as f64 - cy).powi(2)).sqrt();
            if d < r {
                let i = ((y * w + x) * 4) as usize;
                data[i..i + 4].copy_from_slice(&[200, 60, 30, 255]);
            }
        }
    }
    Pixels::new(w, h, data).expect("picture")
}

fn one(kind: EffectKind) -> EffectGraph {
    EffectGraph::new().append(kind).expect("add").0
}

fn bench(c: &mut Criterion) {
    let (w, h) = (1280, 720);
    let source = picture(w, h);
    let space = Space::stage(f64::from(w), f64::from(h));
    for kind in EffectKind::ADDABLE {
        let graph = one(kind);
        c.bench_function(&format!("fx 1280x720 {}", kind.name()), |b| {
            b.iter(|| run(&graph, source.clone(), &space, 12).expect("run"));
        });
    }
    let (w, h) = (1920, 1080);
    let source = picture(w, h);
    let space = Space::stage(f64::from(w), f64::from(h));
    let graph = one(EffectKind::DropShadow)
        .append(EffectKind::Snow)
        .expect("snow")
        .0;
    c.bench_function("fx 1920x1080 drop-shadow then snow", |b| {
        b.iter(|| run(&graph, source.clone(), &space, 12).expect("run"));
    });
}

criterion_group!(benches, bench);
criterion_main!(benches);
