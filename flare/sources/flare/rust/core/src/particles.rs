//! Particle emitters: an object on a layer that sprays dots, squares, stars,
//! a library symbol or a bitmap. Our own design (the role of OpenToonz's
//! particles effect and Animate's Particle System decoration), built to fit
//! the evaluator: nothing is simulated step by step. Particle `i` is born at
//! `i / rate` seconds after the emitter's keyframe with its own random
//! values (a hash of the seed and `i`), and its state at any time is a closed
//! formula — position under constant velocity and gravity, spin, size and
//! colour interpolated over its life, fading in and out. So every frame is
//! independent, scrubbing is exact, and every export (stage, video, GIF,
//! SVG, HTML5, Lottie, sprite sheets) draws the same particles.

use crate::geom::{pt, Matrix, Path, Pt};
use crate::model::{Color, DrawItem, EvalCtx, SymbolKind};
use serde::{Deserialize, Serialize};

/// What each particle looks like.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ParticleSource {
    /// A round dot 10 px across at 100 %.
    Dot,
    /// A 10 px square.
    Square,
    /// A five-pointed star 10 px across.
    Star,
    /// A library symbol (its timeline plays over each particle's life; graphic
    /// symbols loop, the colour tints it by the colour's alpha).
    Symbol { symbol: crate::model::SymbolId },
    /// A library bitmap, centred.
    Bitmap { bitmap: crate::bitmap::BitmapId },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Emitter {
    pub source: ParticleSource,
    /// Particles per second.
    pub rate: f32,
    /// Seconds each particle lives, ± `life_var` (fraction 0…1).
    pub life: f32,
    pub life_var: f32,
    /// Start speed (stage units per second) ± `speed_var` (fraction).
    pub speed: f32,
    pub speed_var: f32,
    /// Direction in degrees (0 = right, −90 = up) and the cone around it.
    pub direction: f32,
    pub spread: f32,
    /// Acceleration (units per second²): +y pulls down.
    pub gravity: Pt,
    /// Air drag 0…1: fraction of speed lost per second.
    pub drag: f32,
    /// Turning speed (degrees per second) ± `spin_var` (degrees per second, random either way).
    pub spin: f32,
    pub spin_var: f32,
    /// Random start angle.
    pub random_angle: bool,
    /// Scale over life, in percent (100 = 10 px shapes / the symbol's own size) ± `scale_var` (fraction).
    pub scale_start: f32,
    pub scale_end: f32,
    pub scale_var: f32,
    /// Colour over life (shapes; symbols and bitmaps use the alpha and a tint by the colour's alpha amount when `tint`).
    pub color_start: Color,
    pub color_end: Color,
    pub tint: bool,
    /// Fade in / out over these fractions of life.
    pub fade_in: f32,
    pub fade_out: f32,
    /// Emission area (width, height) centred on the emitter's origin; 0 × 0 = a point.
    pub area: Pt,
    /// Start as if the emitter had been running for one life already.
    pub prewarm: bool,
    /// Stop emitting after this many seconds (0 = never); live particles finish their lives.
    pub duration: f32,
    /// Most particles drawn at once.
    pub max: u32,
    pub seed: u32,
}

impl Default for Emitter {
    fn default() -> Self {
        Emitter {
            source: ParticleSource::Dot,
            rate: 30.0,
            life: 1.5,
            life_var: 0.3,
            speed: 120.0,
            speed_var: 0.3,
            direction: -90.0,
            spread: 40.0,
            gravity: pt(0.0, 150.0),
            drag: 0.0,
            spin: 0.0,
            spin_var: 90.0,
            random_angle: true,
            scale_start: 100.0,
            scale_end: 30.0,
            scale_var: 0.3,
            color_start: Color::rgb(0xFF, 0xC0, 0x30),
            color_end: Color { r: 0xFF, g: 0x40, b: 0x10, a: 255 },
            tint: false,
            fade_in: 0.05,
            fade_out: 0.4,
            area: pt(20.0, 0.0),
            prewarm: false,
            duration: 0.0,
            max: 600,
            seed: 1,
        }
    }
}

/// One live particle at a moment, in the emitter's coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Particle {
    pub index: u64,
    pub pos: Pt,
    /// Radians.
    pub angle: f32,
    /// 1 = 100 %.
    pub scale: f32,
    pub color: Color,
    /// Fade 0…1 (times the colour's alpha for shapes).
    pub alpha: f32,
    /// Seconds since birth.
    pub age: f32,
}

/// Uniform 0…1 from (seed, particle, channel) — a SplitMix64 finaliser.
fn rnd(seed: u32, i: u64, ch: u64) -> f32 {
    let mut z = (seed as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ i.wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ ch.wrapping_mul(0x94D0_49BB_1331_11EB);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    (z >> 40) as f32 / (1u64 << 24) as f32
}
/// −1…1
fn sym(seed: u32, i: u64, ch: u64) -> f32 {
    rnd(seed, i, ch) * 2.0 - 1.0
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color { r: l(a.r, b.r), g: l(a.g, b.g), b: l(a.b, b.b), a: l(a.a, b.a) }
}

impl Emitter {
    /// Live particles `t` seconds after the emitter appeared, oldest first.
    pub fn particles_at(&self, t: f32) -> Vec<Particle> {
        let rate = self.rate.clamp(0.0, 10_000.0);
        let life = self.life.max(0.01);
        if rate <= 0.0 || !t.is_finite() || t < 0.0 {
            return vec![];
        }
        let max_life = life * (1.0 + self.life_var.clamp(0.0, 1.0));
        let t = if self.prewarm { t + max_life } else { t };
        let born_until = if self.duration > 0.0 { t.min(self.duration + if self.prewarm { max_life } else { 0.0 }) } else { t };
        let last = (born_until * rate).floor() as i64;
        let first = (((t - max_life) * rate).floor() as i64).max(0);
        let mut out = vec![];
        for i in first..=last {
            let i = i as u64;
            let birth = i as f32 / rate;
            let lf = life * (1.0 + self.life_var.clamp(0.0, 1.0) * sym(self.seed, i, 1));
            let age = t - birth;
            if age < 0.0 || age >= lf {
                continue;
            }
            let u = age / lf;
            let origin = pt(self.area.x * 0.5 * sym(self.seed, i, 2), self.area.y * 0.5 * sym(self.seed, i, 3));
            let dir = (self.direction + self.spread * 0.5 * sym(self.seed, i, 4)).to_radians();
            let speed = self.speed * (1.0 + self.speed_var.clamp(0.0, 1.0) * sym(self.seed, i, 5));
            let v = pt(dir.cos() * speed, dir.sin() * speed);
            // velocity decays by drag: ∫ e^(−k s) ds = (1 − e^(−k t)) / k
            let k = -(1.0 - self.drag.clamp(0.0, 0.999)).ln();
            let travel = if k > 1e-6 { (1.0 - (-k * age).exp()) / k } else { age };
            let pos = pt(origin.x + v.x * travel + 0.5 * self.gravity.x * age * age, origin.y + v.y * travel + 0.5 * self.gravity.y * age * age);
            let spin = self.spin + self.spin_var * sym(self.seed, i, 6);
            let a0 = if self.random_angle { std::f32::consts::TAU * rnd(self.seed, i, 7) } else { 0.0 };
            let sv = 1.0 + self.scale_var.clamp(0.0, 1.0) * sym(self.seed, i, 8);
            let scale = ((self.scale_start + (self.scale_end - self.scale_start) * u) / 100.0 * sv).max(0.0);
            let fi = self.fade_in.clamp(0.0, 1.0);
            let fo = self.fade_out.clamp(0.0, 1.0);
            let mut alpha = 1.0f32;
            if fi > 0.0 && u < fi {
                alpha = alpha.min(u / fi);
            }
            if fo > 0.0 && u > 1.0 - fo {
                alpha = alpha.min((1.0 - u) / fo);
            }
            out.push(Particle { index: i, pos, angle: a0 + spin.to_radians() * age, scale, color: lerp_color(self.color_start, self.color_end, u), alpha, age });
        }
        let cap = self.max.clamp(1, 20_000) as usize;
        if out.len() > cap {
            out.drain(..out.len() - cap);
        }
        out
    }

    /// The emission area (or a small square at a point emitter), for
    /// selecting and transforming the emitter on the stage.
    pub fn frame(&self) -> Path {
        let (w, h) = (self.area.x.abs().max(16.0), self.area.y.abs().max(16.0));
        Path::rect(-w / 2.0, -h / 2.0, w, h)
    }

    /// Draw items for the particles at local frame `local`, under `m`.
    #[allow(clippy::too_many_arguments)]
    pub fn eval(&self, ctx: &EvalCtx, m: &Matrix, alpha: f32, tint: Option<(Color, f32)>, local: u32, outline: Option<Color>, depth: u32, out: &mut Vec<DrawItem>) {
        let fps = ctx.doc.fps.max(1.0);
        // the emitter's frame: invisible except in outline mode; keeps it selectable when empty
        out.push(DrawItem { alpha, outline, ..DrawItem::new(self.frame(), *m) });
        if outline.is_some() {
            return;
        }
        for p in self.particles_at(local as f32 / fps) {
            if p.alpha <= 0.0 || p.scale <= 0.0 {
                continue;
            }
            let pm = m.concat(&Matrix::translate(p.pos.x, p.pos.y)).concat(&Matrix::rotate(p.angle)).concat(&Matrix::scale(p.scale, p.scale));
            let a = alpha * p.alpha;
            match self.source {
                ParticleSource::Dot | ParticleSource::Square | ParticleSource::Star => {
                    let path = match self.source {
                        ParticleSource::Dot => Path::ellipse(0.0, 0.0, 5.0, 5.0),
                        ParticleSource::Square => Path::rect(-5.0, -5.0, 10.0, 10.0),
                        _ => Path::star(0.0, 0.0, 5.0, 2.2, 5),
                    };
                    out.push(DrawItem { fill: Some(p.color), alpha: a, tint, ..DrawItem::new(path, pm) });
                }
                ParticleSource::Bitmap { bitmap } => {
                    if let Some(asset) = ctx.doc.bitmap(bitmap) {
                        let (w, h) = (asset.width as f32, asset.height as f32);
                        let t = if self.tint { Some((Color { a: 255, ..p.color }, p.color.a as f32 / 255.0)) } else { tint };
                        out.push(DrawItem { alpha: a * if self.tint { 1.0 } else { p.color.a as f32 / 255.0 }, tint: t, image: asset.image(), ..DrawItem::new(Path::rect(0.0, 0.0, w, h), pm.concat(&Matrix::translate(-w / 2.0, -h / 2.0))) });
                    }
                }
                ParticleSource::Symbol { symbol } => {
                    let Some(sym) = ctx.doc.symbol(symbol) else { continue };
                    if depth > 16 {
                        continue;
                    }
                    let dur = sym.timeline.frame_count().max(1);
                    let age_f = (p.age * fps) as u32;
                    let f = match sym.kind {
                        SymbolKind::Button => 0,
                        _ => age_f % dur,
                    };
                    let first = out.len();
                    // centred on the symbol's registration point
                    crate::model::evaluate(ctx, &sym.timeline, f, &pm, a * if self.tint { 1.0 } else { p.color.a as f32 / 255.0 }, depth + 1, out);
                    let t = if self.tint { Some((Color { a: 255, ..p.color }, p.color.a as f32 / 255.0)) } else { tint };
                    for it in &mut out[first..] {
                        if it.tint.is_none() {
                            it.tint = t;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particles_are_a_pure_function_of_time() {
        let e = Emitter { rate: 10.0, life: 1.0, life_var: 0.0, spread: 0.0, speed: 100.0, speed_var: 0.0, scale_var: 0.0, gravity: pt(0.0, 0.0), area: pt(0.0, 0.0), fade_in: 0.0, fade_out: 0.0, ..Default::default() };
        assert!(e.particles_at(0.0).len() == 1);
        let p = e.particles_at(0.55);
        assert_eq!(p.len(), 6, "born at 0, .1 … .5");
        // the first particle, 0.55 s old, flew straight up at 100/s
        assert!((p[0].pos.y + 55.0).abs() < 1e-3 && p[0].pos.x.abs() < 1e-3, "{:?}", p[0].pos);
        assert_eq!(e.particles_at(0.55), p, "deterministic");
        // steady state: rate × life alive
        assert_eq!(e.particles_at(5.0).len(), 10);
        // prewarm starts full; duration stops emission
        assert_eq!(Emitter { prewarm: true, ..e.clone() }.particles_at(0.0).len(), 10);
        let d = Emitter { duration: 1.0, ..e.clone() };
        assert!(d.particles_at(1.5).len() < 10 && d.particles_at(2.6).is_empty());
        // gravity bends, drag slows, scale and colour change over life
        let g = Emitter { gravity: pt(0.0, 200.0), ..e.clone() };
        assert!((g.particles_at(0.5)[0].pos.y - (-50.0 + 25.0)).abs() < 1e-3);
        let dr = Emitter { drag: 0.9, ..e.clone() };
        assert!(dr.particles_at(0.5)[0].pos.y > -50.0 && dr.particles_at(0.5)[0].pos.y < -20.0);
        let p = &e.particles_at(0.5)[0];
        assert!((p.scale - (1.0 + (0.3 - 1.0) * 0.5)).abs() < 0.2);
        assert!(e.particles_at(1000.0).len() == 10, "late frames stay cheap");
        let capped = Emitter { max: 3, ..e };
        assert_eq!(capped.particles_at(5.0).len(), 3);
    }
}
