//! Offscreen effect groups (filters + blend modes) and mask clipping.

use crate::{draw_level, to_sk_path, to_sk_transform, RenderOpts};
use ac_core::{BevelType, BlendMode, Clip, Color, DrawItem, Filter, FxGroup, Matrix, Pt};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use tiny_skia as sk;

/// Masks built for one draw call, keyed by the clip set and view.
type MaskKey = (Vec<usize>, [i64; 6], u32, u32);

#[derive(Default)]
pub struct MaskCache {
    map: HashMap<MaskKey, Option<Rc<sk::Mask>>>,
}

impl MaskCache {
    pub fn get(&mut self, clips: &[Arc<Clip>], view: &Matrix, w: u32, h: u32) -> Option<Rc<sk::Mask>> {
        let q = |v: f32| (v * 1000.0).round() as i64;
        let key = (clips.iter().map(|c| Arc::as_ptr(c) as usize).collect::<Vec<_>>(), [q(view.a), q(view.b), q(view.c), q(view.d), q(view.tx), q(view.ty)], w, h);
        self.map.entry(key).or_insert_with(|| build_mask(clips, view, w, h).map(Rc::new)).clone()
    }
}

fn build_mask(clips: &[Arc<Clip>], view: &Matrix, w: u32, h: u32) -> Option<sk::Mask> {
    let mut out: Option<sk::Mask> = None;
    for c in clips {
        let mut m = sk::Mask::new(w, h)?;
        for (p, mx) in &c.shapes {
            if let Some(sp) = to_sk_path(p) {
                m.fill_path(&sp, sk::FillRule::Winding, true, to_sk_transform(&view.concat(mx)));
            }
        }
        out = Some(match out {
            None => m,
            Some(mut prev) => {
                for (a, b) in prev.data_mut().iter_mut().zip(m.data()) {
                    *a = ((*a as u16 * *b as u16 + 127) / 255) as u8;
                }
                prev
            }
        });
    }
    out.filter(|m| m.data().iter().any(|v| *v > 0))
}

fn view_scale(m: &Matrix) -> f32 {
    (m.a * m.d - m.b * m.c).abs().sqrt().max(1e-4)
}

/// Device-space bounds of items (strokes included), before filters.
fn device_bounds(items: &[DrawItem], view: &Matrix) -> Option<(f32, f32, f32, f32)> {
    let mut b: Option<(f32, f32, f32, f32)> = None;
    for it in items {
        let m = view.concat(&it.matrix);
        let Some((lo, hi)) = it.path.transformed(&m).bounds() else { continue };
        let pad = it.stroke.map(|s| s.width * view_scale(&m) / 2.0 + 1.0).unwrap_or(1.0);
        let r = (lo.x - pad, lo.y - pad, hi.x + pad, hi.y + pad);
        b = Some(match b {
            None => r,
            Some(a) => (a.0.min(r.0), a.1.min(r.1), a.2.max(r.2), a.3.max(r.3)),
        });
    }
    b
}

pub fn draw_group(pix: &mut sk::PixmapMut, items: &[DrawItem], level: usize, g: &FxGroup, opts: &RenderOpts, masks: &mut MaskCache) {
    let scale = view_scale(&opts.view);
    let Some((mut x0, mut y0, mut x1, mut y1)) = device_bounds(items, &opts.view) else { return };
    // Asset Warp mesh: the layer is drawn deformed, so it may reach further
    let mesh = g.mesh.as_ref().map(|m| m.transformed(&opts.view));
    if let Some((lo, hi)) = mesh.as_ref().and_then(|m| m.dst_bounds()) {
        (x0, y0, x1, y1) = (x0.min(lo.x), y0.min(lo.y), x1.max(hi.x), y1.max(hi.y));
    }
    if let Some((m, frac, _)) = motion_blur(g) {
        if let Some((a0, b0, a1, b1)) = device_bounds(items, &opts.view.concat(&lerp_matrix(&Matrix::IDENTITY, &m, frac))) {
            (x0, y0, x1, y1) = (x0.min(a0), y0.min(b0), x1.max(a1), y1.max(b1));
        }
    }
    let margin = g.filters.iter().map(|f| f.margin()).sum::<f32>() * scale + 2.0;
    let (pw, ph) = (pix.width() as f32, pix.height() as f32);
    let (x0, y0) = ((x0 - margin).floor().max(0.0), (y0 - margin).floor().max(0.0));
    let (x1, y1) = ((x1 + margin).ceil().min(pw), (y1 + margin).ceil().min(ph));
    if x1 <= x0 || y1 <= y0 {
        return;
    }
    let (w, h) = ((x1 - x0) as u32, (y1 - y0) as u32);
    let Some(mut tmp) = sk::Pixmap::new(w, h) else { return };
    let sub = RenderOpts { view: Matrix::translate(-x0, -y0).concat(&opts.view), alpha: opts.alpha, tint: opts.tint };
    match motion_blur(g) {
        Some((motion, frac, samples)) => {
            // Draw the object at `samples` instants over the shutter and average.
            let mut acc = vec![0u32; (w * h * 4) as usize];
            for k in 0..samples {
                let u = (k as f32 + 0.5) / samples as f32 * frac;
                let Some(mut s) = sk::Pixmap::new(w, h) else { return };
                let so = RenderOpts { view: sub.view.concat(&lerp_matrix(&Matrix::IDENTITY, &motion, u)), ..sub };
                draw_level(&mut s.as_mut(), items, level + 1, &so, masks);
                for (a, v) in acc.iter_mut().zip(s.data()) {
                    *a += *v as u32;
                }
            }
            let n = samples;
            for (d, a) in tmp.data_mut().iter_mut().zip(&acc) {
                *d = ((a + n / 2) / n) as u8;
            }
        }
        None => draw_level(&mut tmp.as_mut(), items, level + 1, &sub, masks),
    }
    if let Some(m) = &mesh {
        tmp = warp_mesh(&tmp, m, x0, y0);
    }
    for f in &g.filters {
        match f {
            Filter::Effect { id, params } => {
                crate::effectcraft::apply(&mut tmp, id, params, scale, g.time);
            }
            _ => apply_filter(&mut tmp, f, scale),
        }
    }
    composite(pix, &tmp, x0 as i32, y0 as i32, g.blend);
}

/// Draw `src` (device pixels offset by `x0, y0`) through a deformed mesh:
/// each triangle is filled with the picture mapped by its affine transform.
/// Triangles are grown by half a pixel so neighbours meet without seams.
pub fn warp_mesh(src: &sk::Pixmap, mesh: &ac_core::deform::MeshMap, x0: f32, y0: f32) -> sk::Pixmap {
    let mut out = sk::Pixmap::new(src.width(), src.height()).unwrap_or_else(|| src.clone());
    let off = |p: Pt| ac_core::pt(p.x - x0, p.y - y0);
    for (a, b) in mesh.triangles() {
        let (a, b) = (a.map(off), b.map(off));
        let Some(m) = ac_core::deform::triangle_affine(&a, &b) else { continue };
        let c = ac_core::pt((b[0].x + b[1].x + b[2].x) / 3.0, (b[0].y + b[1].y + b[2].y) / 3.0);
        let grow = |p: Pt| {
            let (dx, dy) = (p.x - c.x, p.y - c.y);
            let l = (dx * dx + dy * dy).sqrt().max(1e-6);
            ac_core::pt(p.x + dx / l * 0.5, p.y + dy / l * 0.5)
        };
        let mut pb = sk::PathBuilder::new();
        let g = b.map(grow);
        pb.move_to(g[0].x, g[0].y);
        pb.line_to(g[1].x, g[1].y);
        pb.line_to(g[2].x, g[2].y);
        pb.close();
        let Some(path) = pb.finish() else { continue };
        let shader = sk::Pattern::new(src.as_ref(), sk::SpreadMode::Pad, sk::FilterQuality::Bilinear, 1.0, sk::Transform::from_row(m.a, m.b, m.c, m.d, m.tx, m.ty));
        let paint = sk::Paint { shader, anti_alias: true, ..Default::default() };
        out.fill_path(&path, &paint, sk::FillRule::Winding, sk::Transform::identity(), None);
    }
    out
}

/// (motion since the previous frame, shutter as a fraction of it, samples)
/// when the group has a Motion Blur filter and the object moved.
fn motion_blur(g: &FxGroup) -> Option<(Matrix, f32, u32)> {
    let m = g.motion?;
    g.filters.iter().find_map(|f| match f {
        Filter::MotionBlur { shutter, samples } if *shutter > 0.0 => Some((m, (shutter / 360.0).min(2.0), (*samples).clamp(2, 64) as u32)),
        _ => None,
    })
}

fn lerp_matrix(a: &Matrix, b: &Matrix, t: f32) -> Matrix {
    let l = |x: f32, y: f32| x + (y - x) * t;
    Matrix { a: l(a.a, b.a), b: l(a.b, b.b), c: l(a.c, b.c), d: l(a.d, b.d), tx: l(a.tx, b.tx), ty: l(a.ty, b.ty) }
}

fn composite(dst: &mut sk::PixmapMut, src: &sk::Pixmap, x: i32, y: i32, blend: BlendMode) {
    let mode = match blend {
        BlendMode::Normal | BlendMode::Layer => sk::BlendMode::SourceOver,
        BlendMode::Darken => sk::BlendMode::Darken,
        BlendMode::Multiply => sk::BlendMode::Multiply,
        BlendMode::Lighten => sk::BlendMode::Lighten,
        BlendMode::Screen => sk::BlendMode::Screen,
        BlendMode::Overlay => sk::BlendMode::Overlay,
        BlendMode::HardLight => sk::BlendMode::HardLight,
        BlendMode::Add => sk::BlendMode::Plus,
        BlendMode::Difference => sk::BlendMode::Difference,
        BlendMode::Alpha => sk::BlendMode::DestinationIn,
        BlendMode::Erase => sk::BlendMode::DestinationOut,
        BlendMode::Subtract | BlendMode::Invert => return composite_manual(dst, src, x, y, blend),
    };
    let paint = sk::PixmapPaint { blend_mode: mode, ..Default::default() };
    dst.draw_pixmap(x, y, src.as_ref(), &paint, sk::Transform::identity(), None);
}

/// Subtract and Invert aren't tiny-skia blend modes: do them per pixel.
fn composite_manual(dst: &mut sk::PixmapMut, src: &sk::Pixmap, x: i32, y: i32, blend: BlendMode) {
    let (dw, dh) = (dst.width() as i32, dst.height() as i32);
    let sw = src.width() as i32;
    let s = src.data();
    let d = dst.data_mut();
    for sy in 0..src.height() as i32 {
        let ty = y + sy;
        if ty < 0 || ty >= dh {
            continue;
        }
        for sx in 0..sw {
            let tx = x + sx;
            if tx < 0 || tx >= dw {
                continue;
            }
            let si = ((sy * sw + sx) * 4) as usize;
            let di = ((ty * dw + tx) * 4) as usize;
            let sa = s[si + 3] as f32 / 255.0;
            if sa <= 0.0 {
                continue;
            }
            for c in 0..3 {
                let dv = d[di + c] as f32;
                let da = d[di + 3] as f32;
                let r = match blend {
                    BlendMode::Subtract => (dv - s[si + c] as f32).max(0.0),
                    _ => da - dv, // Invert: invert the (premultiplied) background
                };
                d[di + c] = (dv + (r - dv) * sa).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

// ---------------------------------------------------------------- filters

/// Separable box blur of one 8-bit plane, `passes` times (≈ Gaussian at 3).
fn blur_plane(p: &mut [f32], w: usize, h: usize, rx: f32, ry: f32, passes: u8) {
    let mut tmp = vec![0.0f32; p.len()];
    for _ in 0..passes.clamp(1, 3) {
        let r = (rx / 2.0).round() as usize;
        if r > 0 {
            for y in 0..h {
                let row = &p[y * w..(y + 1) * w];
                let mut acc = 0.0;
                let n = (2 * r + 1) as f32;
                acc += row.iter().take(r.min(w.saturating_sub(1)) + 1).sum::<f32>();
                for x in 0..w {
                    tmp[y * w + x] = acc / n;
                    if x + r + 1 < w {
                        acc += row[x + r + 1];
                    }
                    if x >= r {
                        acc -= row[x - r];
                    }
                }
            }
            p.copy_from_slice(&tmp);
        }
        let r = (ry / 2.0).round() as usize;
        if r > 0 {
            let n = (2 * r + 1) as f32;
            for x in 0..w {
                let mut acc = 0.0;
                for k in 0..=r.min(h.saturating_sub(1)) {
                    acc += p[k * w + x];
                }
                for y in 0..h {
                    tmp[y * w + x] = acc / n;
                    if y + r + 1 < h {
                        acc += p[(y + r + 1) * w + x];
                    }
                    if y >= r {
                        acc -= p[(y - r) * w + x];
                    }
                }
            }
            p.copy_from_slice(&tmp);
        }
    }
}

fn plane(pix: &sk::Pixmap, c: usize) -> Vec<f32> {
    pix.data().chunks(4).map(|px| px[c] as f32 / 255.0).collect()
}

/// Shift a plane by (dx, dy) pixels (zero fill).
fn shifted(p: &[f32], w: usize, h: usize, dx: i32, dy: i32) -> Vec<f32> {
    let mut out = vec![0.0; p.len()];
    for y in 0..h as i32 {
        let sy = y - dy;
        if sy < 0 || sy >= h as i32 {
            continue;
        }
        for x in 0..w as i32 {
            let sx = x - dx;
            if sx >= 0 && sx < w as i32 {
                out[(y * w as i32 + x) as usize] = p[(sy * w as i32 + sx) as usize];
            }
        }
    }
    out
}

/// A shadow-like alpha plane: outer (from the object's alpha) or inner (from its holes).
#[allow(clippy::too_many_arguments)]
fn shadow_plane(alpha: &[f32], w: usize, h: usize, blur: (f32, f32), quality: u8, angle: f32, distance: f32, strength: f32, inner: bool) -> Vec<f32> {
    let mut src: Vec<f32> = if inner { alpha.iter().map(|a| 1.0 - a).collect() } else { alpha.to_vec() };
    blur_plane(&mut src, w, h, blur.0, blur.1, quality);
    let (dx, dy) = ((angle.to_radians().cos() * distance).round() as i32, (angle.to_radians().sin() * distance).round() as i32);
    let mut s = shifted(&src, w, h, dx, dy);
    for (i, v) in s.iter_mut().enumerate() {
        *v = (*v * strength).clamp(0.0, 1.0);
        if inner {
            *v *= alpha[i];
        }
    }
    s
}

#[derive(Clone, Copy)]
enum Mode {
    /// shadow behind the object
    Under,
    /// shadow painted over the object (inner effects)
    Over,
    /// only the shadow (knockout / hide object)
    Only,
}

fn combine(pix: &mut sk::Pixmap, sh: &[f32], color: Color, mode: Mode) {
    let (cr, cg, cb, ca) = (color.r as f32 / 255.0, color.g as f32 / 255.0, color.b as f32 / 255.0, color.a as f32 / 255.0);
    for (px, s) in pix.data_mut().chunks_mut(4).zip(sh) {
        let a = s * ca;
        let shadow = [cr * a, cg * a, cb * a, a];
        let obj = [px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0, px[3] as f32 / 255.0];
        let out = match mode {
            Mode::Under => [obj[0] + shadow[0] * (1.0 - obj[3]), obj[1] + shadow[1] * (1.0 - obj[3]), obj[2] + shadow[2] * (1.0 - obj[3]), obj[3] + shadow[3] * (1.0 - obj[3])],
            Mode::Over => [shadow[0] + obj[0] * (1.0 - a), shadow[1] + obj[1] * (1.0 - a), shadow[2] + obj[2] * (1.0 - a), obj[3]],
            Mode::Only => shadow,
        };
        for k in 0..4 {
            px[k] = (out[k] * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// Paint a gradient-ramp effect: each pixel takes `ramp(values[i])`, its
/// alpha scaled by `mask[i]`.
fn combine_ramp(pix: &mut sk::Pixmap, values: &[f32], mask: &[f32], stops: &[(f32, Color)], mode: Mode) {
    for ((px, v), m) in pix.data_mut().chunks_mut(4).zip(values).zip(mask) {
        let c = ac_core::fx::ramp(stops, v.clamp(0.0, 1.0));
        let a = c.a as f32 / 255.0 * m.clamp(0.0, 1.0);
        let fx = [c.r as f32 / 255.0 * a, c.g as f32 / 255.0 * a, c.b as f32 / 255.0 * a, a];
        let obj = [px[0] as f32 / 255.0, px[1] as f32 / 255.0, px[2] as f32 / 255.0, px[3] as f32 / 255.0];
        let out = match mode {
            Mode::Under => [obj[0] + fx[0] * (1.0 - obj[3]), obj[1] + fx[1] * (1.0 - obj[3]), obj[2] + fx[2] * (1.0 - obj[3]), obj[3] + fx[3] * (1.0 - obj[3])],
            Mode::Over => [fx[0] + obj[0] * (1.0 - a), fx[1] + obj[1] * (1.0 - a), fx[2] + obj[2] * (1.0 - a), obj[3]],
            Mode::Only => fx,
        };
        for k in 0..4 {
            px[k] = (out[k] * 255.0).round().clamp(0.0, 255.0) as u8;
        }
    }
}

pub fn apply_filter(pix: &mut sk::Pixmap, f: &Filter, scale: f32) {
    let (w, h) = (pix.width() as usize, pix.height() as usize);
    if let Filter::GradientGlow { blur_x, blur_y, strength, quality, angle, distance, knockout, kind, ref stops } = *f {
        let alpha = plane(pix, 3);
        let b = (blur_x * scale, blur_y * scale);
        let st = strength / 100.0;
        if knockout {
            pix.data_mut().fill(0);
        }
        let mode = |under| if knockout { Mode::Only } else if under { Mode::Under } else { Mode::Over };
        if kind != BevelType::Inner {
            let g = shadow_plane(&alpha, w, h, b, quality, angle, distance * scale, st, false);
            let mask: Vec<f32> = g.iter().zip(&alpha).map(|(v, a)| if knockout { (1.0 - a) * v.min(1.0).ceil() } else { v.min(1.0).ceil() }).collect();
            combine_ramp(pix, &g, &mask, stops, mode(true));
        }
        if kind != BevelType::Outer {
            let g = shadow_plane(&alpha, w, h, b, quality, angle, distance * scale, st, true);
            combine_ramp(pix, &g, &alpha, stops, mode(false));
        }
        return;
    }
    if let Filter::GradientBevel { blur_x, blur_y, strength, quality, angle, distance, knockout, kind, ref stops } = *f {
        let alpha = plane(pix, 3);
        let b = (blur_x * scale, blur_y * scale);
        let (d, st) = (distance * scale, strength / 100.0);
        if knockout {
            pix.data_mut().fill(0);
        }
        // 0 = highlight side, 0.5 = flat, 1 = shadow side
        let side = |inner: bool| -> Vec<f32> {
            let lit = shadow_plane(&alpha, w, h, b, quality, angle + 180.0, d, st, inner);
            let dark = shadow_plane(&alpha, w, h, b, quality, angle, d, st, inner);
            lit.iter().zip(&dark).map(|(l, k)| 0.5 + 0.5 * (if inner { l - k } else { k - l })).collect()
        };
        if kind != BevelType::Outer {
            let v = side(true);
            combine_ramp(pix, &v, &alpha, stops, if knockout { Mode::Only } else { Mode::Over });
        }
        if kind != BevelType::Inner {
            let v = side(false);
            let mask: Vec<f32> = alpha.iter().map(|a| 1.0 - a).collect();
            combine_ramp(pix, &v, &mask, stops, if knockout { Mode::Only } else { Mode::Under });
        }
        return;
    }
    match *f {
        Filter::GradientGlow { .. } | Filter::GradientBevel { .. } => {}
        Filter::Blur { blur_x, blur_y, quality } => {
            let mut planes: Vec<Vec<f32>> = (0..4).map(|c| plane(pix, c)).collect();
            for p in &mut planes {
                blur_plane(p, w, h, blur_x * scale, blur_y * scale, quality);
            }
            for (i, px) in pix.data_mut().chunks_mut(4).enumerate() {
                let a = (planes[3][i] * 255.0).round().clamp(0.0, 255.0);
                px[3] = a as u8;
                for c in 0..3 {
                    px[c] = (planes[c][i] * 255.0).round().clamp(0.0, a) as u8;
                }
            }
        }
        Filter::DropShadow { blur_x, blur_y, strength, quality, angle, distance, color, knockout, inner, hide_object } => {
            let alpha = plane(pix, 3);
            let sh = shadow_plane(&alpha, w, h, (blur_x * scale, blur_y * scale), quality, angle, distance * scale, strength / 100.0, inner);
            let mode = if hide_object && !inner {
                Mode::Only
            } else if inner {
                if knockout { Mode::Only } else { Mode::Over }
            } else if knockout {
                // outer knockout: shadow with the object cut out
                let mut s2 = sh.clone();
                for (v, a) in s2.iter_mut().zip(&alpha) {
                    *v *= 1.0 - a;
                }
                combine(pix, &s2, color, Mode::Only);
                return;
            } else {
                Mode::Under
            };
            combine(pix, &sh, color, mode);
        }
        Filter::Glow { blur_x, blur_y, strength, quality, color, knockout, inner } => {
            apply_filter(pix, &Filter::DropShadow { blur_x, blur_y, strength, quality, angle: 0.0, distance: 0.0, color, knockout, inner, hide_object: false }, scale);
        }
        Filter::Bevel { blur_x, blur_y, strength, quality, shadow, highlight, angle, distance, knockout, kind } => {
            let alpha = plane(pix, 3);
            let b = (blur_x * scale, blur_y * scale);
            let d = distance * scale;
            let st = strength / 100.0;
            let inner = kind != BevelType::Outer;
            let outer = kind != BevelType::Inner;
            if knockout {
                for v in pix.data_mut() {
                    *v = 0;
                }
            }
            if inner {
                let hi = shadow_plane(&alpha, w, h, b, quality, angle + 180.0, d, st, true);
                let lo = shadow_plane(&alpha, w, h, b, quality, angle, d, st, true);
                combine(pix, &hi, shadow, Mode::Over);
                combine(pix, &lo, highlight, Mode::Over);
            }
            if outer {
                let hi = shadow_plane(&alpha, w, h, b, quality, angle + 180.0, d, st, false);
                let lo = shadow_plane(&alpha, w, h, b, quality, angle, d, st, false);
                combine(pix, &hi, highlight, Mode::Under);
                combine(pix, &lo, shadow, Mode::Under);
            }
        }
        Filter::AdjustColor { brightness, contrast, saturation, hue } => adjust_color(pix, brightness, contrast, saturation, hue),
        // Motion blur happens while drawing the group (draw_group).
        Filter::MotionBlur { .. } => {}
        Filter::Effect { ref id, ref params } => {
            crate::effectcraft::apply(pix, id, params, scale, 0.0);
        }
    }
}

/// Animate's Adjust Color: brightness/contrast/saturation (−100…100) and hue (±180°).
fn adjust_color(pix: &mut sk::Pixmap, brightness: f32, contrast: f32, saturation: f32, hue: f32) {
    let b = brightness.clamp(-100.0, 100.0) / 100.0;
    let c = 1.0 + contrast.clamp(-100.0, 100.0) / 100.0;
    let s = 1.0 + saturation.clamp(-100.0, 100.0) / 100.0;
    let (sin, cos) = hue.to_radians().sin_cos();
    // luminance-preserving hue rotation matrix
    let (lr, lg, lb) = (0.213, 0.715, 0.072);
    let m = [
        [lr + cos * (1.0 - lr) + sin * (-lr), lg + cos * (-lg) + sin * (-lg), lb + cos * (-lb) + sin * (1.0 - lb)],
        [lr + cos * (-lr) + sin * 0.143, lg + cos * (1.0 - lg) + sin * 0.140, lb + cos * (-lb) + sin * (-0.283)],
        [lr + cos * (-lr) + sin * (-(1.0 - lr)), lg + cos * (-lg) + sin * lg, lb + cos * (1.0 - lb) + sin * lb],
    ];
    for px in pix.data_mut().chunks_mut(4) {
        let a = px[3] as f32 / 255.0;
        if a <= 0.0 {
            continue;
        }
        let mut rgb = [px[0] as f32 / 255.0 / a, px[1] as f32 / 255.0 / a, px[2] as f32 / 255.0 / a];
        rgb = [m[0][0] * rgb[0] + m[0][1] * rgb[1] + m[0][2] * rgb[2], m[1][0] * rgb[0] + m[1][1] * rgb[1] + m[1][2] * rgb[2], m[2][0] * rgb[0] + m[2][1] * rgb[1] + m[2][2] * rgb[2]];
        let l = lr * rgb[0] + lg * rgb[1] + lb * rgb[2];
        for v in &mut rgb {
            *v = l + (*v - l) * s;
            *v = (*v - 0.5) * c + 0.5;
            *v += b;
            *v = v.clamp(0.0, 1.0);
        }
        for k in 0..3 {
            px[k] = (rgb[k] * a * 255.0).round() as u8;
        }
    }
}
