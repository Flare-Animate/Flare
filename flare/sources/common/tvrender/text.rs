//! Static text → vector outlines (ab_glyph). Text stays editable in the
//! document; outlines are produced at evaluation time, so it renders and
//! exports exactly like shapes.

use crate::geom::{pt, Path, Pt, Seg};
use ab_glyph::{Font, FontRef, FontVec, OutlineCurve, ScaleFont};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FontFace {
    /// Ubuntu Light (Ubuntu Font Licence), bundled.
    #[default]
    Sans,
    /// Hack (MIT/Bitstream Vera), bundled.
    Mono,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

fn font(face: FontFace) -> &'static FontRef<'static> {
    static SANS: OnceLock<FontRef<'static>> = OnceLock::new();
    static MONO: OnceLock<FontRef<'static>> = OnceLock::new();
    match face {
        FontFace::Sans => SANS.get_or_init(|| FontRef::try_from_slice(epaint_default_fonts::UBUNTU_LIGHT).expect("bundled font")),
        FontFace::Mono => MONO.get_or_init(|| FontRef::try_from_slice(epaint_default_fonts::HACK_REGULAR).expect("bundled font")),
    }
}

/// Font loader: (family, weight 100–900, italic) → (font file, its weight, its italic).
type Loader = fn(&str, u16, bool) -> Option<(Vec<u8>, u16, bool)>;
static LOADER: OnceLock<Loader> = OnceLock::new();

/// Let the host provide font files by family name (the desktop app and CLI
/// install a system-font loader; the core stays free of OS font code).
pub fn set_font_loader(f: Loader) {
    let _ = LOADER.set(f);
}

/// A font by family name: explicitly registered, or found by the loader.
pub fn family_font(family: &str) -> Option<Arc<FontVec>> {
    styled_font(family, 400, false).map(|(f, _, _)| f)
}

/// The closest face of `family` to the weight/italic asked for, with the
/// weight and italic it actually has.
pub fn styled_font(family: &str, weight: u16, italic: bool) -> Option<(Arc<FontVec>, u16, bool)> {
    type Cache = Mutex<HashMap<(String, u16, bool), Option<(Arc<FontVec>, u16, bool)>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (family.to_string(), weight, italic);
    if let Some(f) = cache.lock().ok()?.get(&key) {
        return f.clone();
    }
    let f = LOADER.get().and_then(|l| l(family, weight, italic)).and_then(|(b, w, i)| FontVec::try_from_vec(b).ok().map(|f| (Arc::new(f), w, i)));
    cache.lock().ok()?.insert(key, f.clone());
    f
}

/// Outline with a weight and italic: uses the matching face when the family
/// has one, otherwise fakes them (bold grows the outline, italic slants it).
#[allow(clippy::too_many_arguments)]
pub fn text_path_styled(text: &str, size: f32, family: Option<&str>, face: FontFace, weight: u16, italic: bool, align: Align, letter_spacing: f32) -> Path {
    let (path, have_w, have_i, ascent) = match family.and_then(|fam| styled_font(fam, weight, italic)) {
        Some((f, w, i)) => (outline_with(&*f, text, size, align, letter_spacing), w, i, f.as_scaled(size.max(1.0)).ascent()),
        None => {
            let base_w = if face == FontFace::Sans { 300 } else { 400 };
            (text_path(text, size, face, align, letter_spacing), base_w, false, font(face).as_scaled(size.max(1.0)).ascent())
        }
    };
    let mut path = path;
    let extra = weight as f32 - have_w as f32;
    if extra >= 150.0 {
        // faux bold: grow the glyphs (about 3.5% of the size from regular to bold)
        let r = size * 0.035 * (extra / 300.0).min(1.5) / 2.0;
        let polys = crate::merge::path_polygons(&path);
        let grown = crate::merge::grow_polygons(&polys, r);
        if !grown.is_empty() {
            path = crate::merge::polys_path(&grown);
        }
    }
    if italic && !have_i {
        // faux italic: slant 12° about the first baseline
        let k = 12f32.to_radians().tan();
        path = path.transformed(&crate::geom::Matrix { a: 1.0, b: 0.0, c: -k, d: 1.0, tx: k * ascent, ty: 0.0 });
    }
    path
}

/// Outline `text` with its top-left at (0, 0). Lines split on '\n'.
pub fn text_path(text: &str, size: f32, face: FontFace, align: Align, letter_spacing: f32) -> Path {
    outline_with(font(face), text, size, align, letter_spacing)
}

/// Like `text_path`, using a named (system) font family when available.
pub fn text_path_family(text: &str, size: f32, family: Option<&str>, face: FontFace, align: Align, letter_spacing: f32) -> Path {
    match family.and_then(family_font) {
        Some(f) => outline_with(&*f, text, size, align, letter_spacing),
        None => text_path(text, size, face, align, letter_spacing),
    }
}

fn outline_with<F: Font>(font: &F, text: &str, size: f32, align: Align, letter_spacing: f32) -> Path {
    let f = font.as_scaled(size.max(1.0));
    let scale = f.scale_factor();
    let line_h = f.height() + f.line_gap();
    let lines: Vec<&str> = text.split('\n').collect();
    let widths: Vec<f32> = lines.iter().map(|l| line_width(&f, l, letter_spacing)).collect();
    let max_w = widths.iter().cloned().fold(0.0, f32::max);
    let mut segs = Vec::new();
    for (li, line) in lines.iter().enumerate() {
        let x0 = match align {
            Align::Left => 0.0,
            Align::Center => (max_w - widths[li]) / 2.0,
            Align::Right => max_w - widths[li],
        };
        let baseline = f.ascent() + li as f32 * line_h;
        let mut x = x0;
        let mut prev: Option<ab_glyph::GlyphId> = None;
        for ch in line.chars() {
            let id = f.glyph_id(ch);
            if let Some(p) = prev {
                x += f.kern(p, id);
            }
            if let Some(outline) = f.font().outline(id) {
                // Each glyph contour is a chain of curves; start a new subpath at breaks.
                let mut cur: Option<ab_glyph::Point> = None;
                let tf = |p: ab_glyph::Point| pt(x + p.x * scale.horizontal, baseline - p.y * scale.vertical);
                for c in &outline.curves {
                    let (a, end) = match *c {
                        OutlineCurve::Line(a, b) => (a, b),
                        OutlineCurve::Quad(a, _, b) => (a, b),
                        OutlineCurve::Cubic(a, _, _, b) => (a, b),
                    };
                    if cur.map(|p| (p.x - a.x).abs() > 1e-3 || (p.y - a.y).abs() > 1e-3).unwrap_or(true) {
                        if cur.is_some() {
                            segs.push(Seg::Z);
                        }
                        segs.push(Seg::M(tf(a)));
                    }
                    segs.push(match *c {
                        OutlineCurve::Line(_, b) => Seg::L(tf(b)),
                        OutlineCurve::Quad(_, q, b) => Seg::Q(tf(q), tf(b)),
                        OutlineCurve::Cubic(_, q1, q2, b) => Seg::C(tf(q1), tf(q2), tf(b)),
                    });
                    cur = Some(end);
                }
                if cur.is_some() {
                    segs.push(Seg::Z);
                }
            }
            x += f.h_advance(id) + letter_spacing;
            prev = Some(id);
        }
    }
    Path { segs }
}

/// Top-left position of every visible character (Break Apart: one text
/// object per letter), same layout as `text_path`.
pub fn letter_positions(text: &str, size: f32, family: Option<&str>, face: FontFace, align: Align, letter_spacing: f32) -> Vec<(char, Pt)> {
    match family.and_then(family_font) {
        Some(f) => letters_with(&*f, text, size, align, letter_spacing),
        None => letters_with(font(face), text, size, align, letter_spacing),
    }
}

fn letters_with<F: Font>(font: &F, text: &str, size: f32, align: Align, letter_spacing: f32) -> Vec<(char, Pt)> {
    let f = font.as_scaled(size.max(1.0));
    let line_h = f.height() + f.line_gap();
    let lines: Vec<&str> = text.split('\n').collect();
    let widths: Vec<f32> = lines.iter().map(|l| line_width(&f, l, letter_spacing)).collect();
    let max_w = widths.iter().cloned().fold(0.0, f32::max);
    let mut out = vec![];
    for (li, line) in lines.iter().enumerate() {
        let mut x = match align {
            Align::Left => 0.0,
            Align::Center => (max_w - widths[li]) / 2.0,
            Align::Right => max_w - widths[li],
        };
        let mut prev: Option<ab_glyph::GlyphId> = None;
        for ch in line.chars() {
            let id = f.glyph_id(ch);
            if let Some(p) = prev {
                x += f.kern(p, id);
            }
            if !ch.is_whitespace() {
                out.push((ch, pt(x, li as f32 * line_h)));
            }
            x += f.h_advance(id) + letter_spacing;
            prev = Some(id);
        }
    }
    out
}

fn line_width<F: Font, S: ScaleFont<F>>(f: &S, line: &str, spacing: f32) -> f32 {
    let mut w = 0.0;
    let mut prev = None;
    for ch in line.chars() {
        let id = f.glyph_id(ch);
        if let Some(p) = prev {
            w += f.kern(p, id);
        }
        w += f.h_advance(id) + spacing;
        prev = Some(id);
    }
    w
}
