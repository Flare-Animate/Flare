//! Renderer-neutral deterministic scenes. Same bytes for every backend and run.

#[derive(Clone, Copy)]
pub enum Seg {
    Move(f32, f32),
    Line(f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

pub struct Item {
    pub segs: Vec<Seg>,
    pub rgba: [u8; 4],
    /// `Some(width)` strokes with round caps and joins; `None` fills non-zero.
    pub stroke: Option<f32>,
}

struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / ((1u64 << 24) as f32)
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }

    fn byte(&mut self) -> u8 {
        (self.next() * 255.0) as u8
    }
}

pub fn build(name: &str, w: f32, h: f32) -> Option<Vec<Item>> {
    let mut rng = Lcg(0x5EED_F1A4E);
    let mut items = Vec::new();
    match name {
        // Opaque triangles sharing every edge: exposes conflation seams.
        "adjacent" => {
            let n = 40;
            let (cw, ch) = (w / n as f32, h / n as f32);
            for gy in 0..n {
                for gx in 0..n {
                    let (x0, y0) = (gx as f32 * cw, gy as f32 * ch);
                    let (x1, y1) = (x0 + cw, y0 + ch);
                    for tri in [[(x0, y0), (x1, y0), (x0, y1)], [(x1, y0), (x1, y1), (x0, y1)]] {
                        let color = [rng.byte(), rng.byte(), rng.byte(), 255];
                        items.push(Item {
                            segs: vec![
                                Seg::Move(tri[0].0, tri[0].1),
                                Seg::Line(tri[1].0, tri[1].1),
                                Seg::Line(tri[2].0, tri[2].1),
                                Seg::Close,
                            ],
                            rgba: color,
                            stroke: None,
                        });
                    }
                }
            }
        }
        // Translucent closed cubic blobs with jittered handles (some self-intersect).
        "blobs" => {
            for _ in 0..1500 {
                let (cx, cy, r) = (rng.range(0.0, w), rng.range(0.0, h), rng.range(10.0, 120.0));
                let radii: Vec<f32> = (0..4).map(|_| rng.range(0.5 * r, r)).collect();
                let anchor = |i: usize| {
                    let t = i as f32 * std::f32::consts::FRAC_PI_2;
                    (cx + radii[i % 4] * t.cos(), cy + radii[i % 4] * t.sin(), t)
                };
                let mut segs = Vec::new();
                let (x0, y0, _) = anchor(0);
                segs.push(Seg::Move(x0, y0));
                for i in 0..4 {
                    let (ax, ay, at) = anchor(i);
                    let (bx, by, bt) = anchor(i + 1);
                    let k = 0.5523;
                    let j = 0.4 * r;
                    segs.push(Seg::Cubic(
                        ax - k * radii[i % 4] * at.sin() + rng.range(-j, j),
                        ay + k * radii[i % 4] * at.cos() + rng.range(-j, j),
                        bx + k * radii[(i + 1) % 4] * bt.sin() + rng.range(-j, j),
                        by - k * radii[(i + 1) % 4] * bt.cos() + rng.range(-j, j),
                        bx,
                        by,
                    ));
                }
                segs.push(Seg::Close);
                items.push(Item { segs, rgba: [rng.byte(), rng.byte(), rng.byte(), 150], stroke: None });
            }
        }
        // Thick round-capped curves.
        "strokes" => {
            for _ in 0..1500 {
                let p = |rng: &mut Lcg| (rng.range(0.0, w), rng.range(0.0, h));
                let (a, b, c, d) = (p(&mut rng), p(&mut rng), p(&mut rng), p(&mut rng));
                items.push(Item {
                    segs: vec![Seg::Move(a.0, a.1), Seg::Cubic(b.0, b.1, c.0, c.1, d.0, d.1)],
                    rgba: [rng.byte(), rng.byte(), rng.byte(), 255],
                    stroke: Some(rng.range(1.0, 24.0)),
                });
            }
        }
        _ => return None,
    }
    Some(items)
}
