//! Raster painting with MyPaint brushes (Properties ▸ Tool ▸ Raster Brush),
//! as OpenToonz does with libmypaint (`tnztools/toonzrasterbrushtool.cpp`,
//! `mypainttoonzbrush.cpp`). The brush engine is **hokusai**, a pure-Rust,
//! pixel-faithful port of libmypaint (MIT OR Apache-2.0), used as a crates.io
//! dependency; the brushes are a CC0 selection of the MyPaint brush pack
//! (`assets/mypaint/`). Strokes paint into a library bitmap: the bitmap's
//! pixels go into a tiled linear-light surface, the brush paints dabs, and
//! the touched tiles come back out as sRGB.

use crate::bitmap::Image;
use crate::model::Color;
use hokusai::color::{linear_to_srgb, rgb_to_hsv, srgb_to_linear};
use hokusai::fix15;
use hokusai::mapping::SettingValue;
use hokusai::tile::TILE_SIZE;
use hokusai::tile_mem::MemSurface;
use hokusai::{Brush, BrushSetting, BrushState, TiledSurface};
use std::sync::Arc;

/// A bundled brush.
#[derive(Clone, Copy, Debug)]
pub struct Preset {
    /// File stem in `assets/mypaint` (script id).
    pub id: &'static str,
    pub name: &'static str,
    pub group: &'static str,
    myb: &'static str,
}

macro_rules! preset {
    ($id:literal, $name:literal, $group:literal) => {
        Preset { id: $id, name: $name, group: $group, myb: include_str!(concat!("../rust/ac_core/assets/mypaint/", $id, ".myb")) }
    };
}

pub const PRESETS: &[Preset] = &[
    preset!("classic_brush", "Brush", "Classic"),
    preset!("classic_calligraphy", "Calligraphy", "Classic"),
    preset!("classic_charcoal", "Charcoal", "Classic"),
    preset!("classic_dry_brush", "Dry brush", "Classic"),
    preset!("classic_impressionism", "Impressionism", "Classic"),
    preset!("classic_ink_blot", "Ink blot", "Classic"),
    preset!("classic_ink_eraser", "Ink eraser", "Classic"),
    preset!("classic_knife", "Knife", "Classic"),
    preset!("classic_long_grass", "Long grass", "Classic"),
    preset!("classic_marker_fat", "Marker fat", "Classic"),
    preset!("classic_pen", "Pen", "Classic"),
    preset!("classic_pencil", "Pencil", "Classic"),
    preset!("classic_puantilism", "Puantilism", "Classic"),
    preset!("classic_smudge", "Smudge", "Classic"),
    preset!("classic_textured_ink", "Textured ink", "Classic"),
    preset!("deevad_2B_pencil", "2B pencil", "David Revoy"),
    preset!("deevad_airbrush", "Airbrush", "David Revoy"),
    preset!("deevad_ballpen", "Ballpen", "David Revoy"),
    preset!("deevad_chalk", "Chalk", "David Revoy"),
    preset!("deevad_kneaded_eraser", "Kneaded eraser", "David Revoy"),
    preset!("deevad_liner", "Liner", "David Revoy"),
    preset!("deevad_rough", "Rough", "David Revoy"),
    preset!("deevad_spray", "Spray", "David Revoy"),
    preset!("deevad_thin_watercolor", "Thin watercolor", "David Revoy"),
    preset!("deevad_watercolor_expressive", "Watercolor expressive", "David Revoy"),
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id || p.name.eq_ignore_ascii_case(id))
}

impl Preset {
    /// The parsed brush (as authored).
    pub fn brush(&self) -> Result<Brush, String> {
        hokusai::myb::from_str(self.myb).map_err(|e| format!("{}: {e:?}", self.id))
    }
    /// The brush's own radius (px) and whether it erases.
    pub fn base_radius(&self) -> f32 {
        self.brush().map(|b| b.get(BrushSetting::Radius).base_value.exp()).unwrap_or(2.0)
    }
}

/// Tool options for a stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintOpts {
    pub color: Color,
    /// Radius in bitmap pixels (`None` = the brush's own).
    pub radius: Option<f32>,
    /// Multiplies the brush's opacity (0…1).
    pub opacity: f32,
    /// Mouse/pen pressure used when the device reports none.
    pub pressure: f32,
    /// Paint transparency instead of colour.
    pub eraser: bool,
}

impl Default for PaintOpts {
    fn default() -> Self {
        PaintOpts { color: Color::rgb(0, 0, 0), radius: None, opacity: 1.0, pressure: 0.5, eraser: false }
    }
}

/// A brush with the tool's colour, size and opacity applied.
pub fn configure(p: &Preset, o: &PaintOpts) -> Result<Brush, String> {
    let mut b = p.brush()?;
    // brush colours are HSV of linear-light RGB (hokusai / libmypaint linear mode)
    let lin = |c: u8| srgb_to_linear(c as f32 / 255.0);
    let hsv = rgb_to_hsv(lin(o.color.r), lin(o.color.g), lin(o.color.b));
    b.get_mut(BrushSetting::ColorH).base_value = hsv.h;
    b.get_mut(BrushSetting::ColorS).base_value = hsv.s;
    b.get_mut(BrushSetting::ColorV).base_value = hsv.v;
    if let Some(r) = o.radius {
        b.get_mut(BrushSetting::Radius).base_value = r.clamp(0.2, 500.0).ln();
    }
    let op = b.get(BrushSetting::Opaque).base_value;
    b.get_mut(BrushSetting::Opaque).base_value = op * o.opacity.clamp(0.0, 1.0) * (o.color.a as f32 / 255.0);
    if o.eraser {
        b.set(BrushSetting::Eraser, SettingValue::constant(1.0));
    }
    Ok(b)
}

/// One stroke painting into a bitmap of `width` × `height`.
pub struct PaintStroke {
    brush: Brush,
    state: BrushState,
    surface: MemSurface,
    pub width: u32,
    pub height: u32,
    pressure: f32,
    /// Touched area so far: (x0, y0, x1, y1) in pixels, exclusive end.
    pub dirty: Option<(u32, u32, u32, u32)>,
}

impl PaintStroke {
    /// Start from the bitmap's current pixels (premultiplied sRGB, `Image` layout).
    #[allow(clippy::needless_range_loop)] // tile and image indices run together
    pub fn new(img: &Image, p: &Preset, o: &PaintOpts) -> Result<PaintStroke, String> {
        let brush = configure(p, o)?;
        let mut surface = MemSurface::new();
        let ts = TILE_SIZE as u32;
        for ty in 0..img.height.div_ceil(ts) {
            for tx in 0..img.width.div_ceil(ts) {
                let mut any = false;
                let t = surface.tile_request_start(tx as i32, ty as i32);
                for ly in 0..TILE_SIZE {
                    for lx in 0..TILE_SIZE {
                        let (x, y) = (tx * ts + lx as u32, ty * ts + ly as u32);
                        if x >= img.width || y >= img.height {
                            continue;
                        }
                        let i = ((y * img.width + x) * 4) as usize;
                        let a = img.rgba[i + 3] as f32 / 255.0;
                        if a <= 0.0 {
                            continue;
                        }
                        any = true;
                        // premultiplied sRGB → straight → linear → premultiplied fix15
                        let c = |k: usize| fix15::from_f32(srgb_to_linear((img.rgba[i + k] as f32 / 255.0 / a).min(1.0)) * a);
                        t[ly][lx] = [c(0), c(1), c(2), fix15::from_f32(a)];
                    }
                }
                surface.tile_request_end(tx as i32, ty as i32);
                let _ = any;
            }
        }
        Ok(PaintStroke { brush, state: BrushState::default(), surface, width: img.width, height: img.height, pressure: o.pressure, dirty: None })
    }

    /// Move the brush to (x, y) in bitmap pixels; `pressure` None = the default.
    pub fn to(&mut self, x: f32, y: f32, pressure: Option<f32>, dtime: f64) -> bool {
        self.surface.begin_atomic();
        let painted = self.brush.stroke_to(&mut self.state, &mut self.surface, x, y, pressure.unwrap_or(self.pressure), 0.0, 0.0, dtime.max(0.001));
        for (tx, ty) in self.surface.end_atomic() {
            if tx < 0 || ty < 0 {
                continue;
            }
            let ts = TILE_SIZE as u32;
            let (x0, y0) = ((tx as u32 * ts).min(self.width), (ty as u32 * ts).min(self.height));
            let (x1, y1) = (((tx as u32 + 1) * ts).min(self.width), ((ty as u32 + 1) * ts).min(self.height));
            if x0 >= x1 || y0 >= y1 {
                continue;
            }
            self.dirty = Some(match self.dirty {
                Some((a, b, c, d)) => (a.min(x0), b.min(y0), c.max(x1), d.max(y1)),
                None => (x0, y0, x1, y1),
            });
        }
        painted
    }

    /// Write the touched area back into premultiplied sRGB pixels.
    pub fn write_into(&self, img: &mut Image) {
        let Some((x0, y0, x1, y1)) = self.dirty else { return };
        let ts = TILE_SIZE as u32;
        for y in y0..y1.min(img.height) {
            for x in x0..x1.min(img.width) {
                let i = ((y * img.width + x) * 4) as usize;
                let Some(t) = self.surface.tile((x / ts) as i32, (y / ts) as i32) else {
                    img.rgba[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
                    continue;
                };
                let p = t[(y % ts) as usize][(x % ts) as usize];
                let a = fix15::to_f32(p[3]).clamp(0.0, 1.0);
                if a <= 0.0 {
                    img.rgba[i..i + 4].copy_from_slice(&[0, 0, 0, 0]);
                    continue;
                }
                let c = |k: usize| ((linear_to_srgb((fix15::to_f32(p[k]) / a).clamp(0.0, 1.0)) * a * 255.0).round().clamp(0.0, 255.0)) as u8;
                img.rgba[i..i + 4].copy_from_slice(&[c(0), c(1), c(2), (a * 255.0).round() as u8]);
            }
        }
    }
}

/// Straight (non-premultiplied) RGBA from premultiplied, for PNG encoding.
pub fn unpremultiply(img: &Image) -> Vec<u8> {
    let mut v = img.rgba.clone();
    for px in v.chunks_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut px[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    v
}

/// A new blank (transparent) image.
pub fn blank(width: u32, height: u32) -> Arc<Image> {
    Arc::new(Image { width, height, rgba: vec![0; (width * height * 4) as usize], smoothing: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_brush_parses_and_paints() {
        for p in PRESETS {
            let mut img = Image { width: 120, height: 60, rgba: vec![0; 120 * 60 * 4], smoothing: true };
            let o = PaintOpts { color: Color::rgb(200, 30, 30), radius: Some(6.0), ..Default::default() };
            let mut s = PaintStroke::new(&img, p, &o).unwrap_or_else(|e| panic!("{e}"));
            for i in 0..=60 {
                s.to(10.0 + i as f32 * 1.6, 30.0, Some(0.7), 0.01);
            }
            s.write_into(&mut img);
            let painted = img.rgba.chunks(4).filter(|px| px[3] > 0).count();
            if p.id.contains("eraser") || p.id.contains("smudge") || p.id.contains("knife") {
                continue; // nothing to erase / smudge on a blank bitmap
            }
            assert!(painted > 50, "{} painted {painted} px", p.id);
        }
    }

    #[test]
    fn colour_comes_out_as_chosen_and_erasing_clears() {
        let p = preset("classic_pen").unwrap();
        let mut img = Image { width: 64, height: 64, rgba: vec![0; 64 * 64 * 4], smoothing: true };
        let o = PaintOpts { color: Color::rgb(40, 120, 220), radius: Some(5.0), pressure: 1.0, ..Default::default() };
        let mut s = PaintStroke::new(&img, p, &o).unwrap();
        for i in 0..=40 {
            s.to(10.0 + i as f32, 32.0, None, 0.01);
        }
        s.write_into(&mut img);
        let i = ((32 * 64 + 30) * 4) as usize;
        let px = &img.rgba[i..i + 4];
        assert!(px[3] > 200, "solid in the middle: {px:?}");
        let (r, g, b) = (px[0] as i32 * 255 / px[3] as i32, px[1] as i32 * 255 / px[3] as i32, px[2] as i32 * 255 / px[3] as i32);
        assert!((r - 40).abs() < 12 && (g - 120).abs() < 12 && (b - 220).abs() < 12, "{r} {g} {b}");
        // erase across it
        let mut e = PaintStroke::new(&img, p, &PaintOpts { eraser: true, radius: Some(8.0), pressure: 1.0, ..o }).unwrap();
        for k in 0..=20 {
            e.to(30.0, 20.0 + k as f32, None, 0.01);
        }
        e.write_into(&mut img);
        assert!(img.rgba[i + 3] < 40, "erased: {}", img.rgba[i + 3]);
        assert!(img.rgba[((32 * 64 + 12) * 4 + 3) as usize] > 200, "the rest stays");
    }
}
