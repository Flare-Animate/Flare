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
    /// Lines of a fixed-width box fill the width (the last line of each paragraph stays left).
    Justify,
}

/// Character position: normal, superscript or subscript (smaller, raised or lowered).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Baseline {
    #[default]
    Normal,
    Superscript,
    Subscript,
}

/// Paragraph and character layout options (Properties › Paragraph / Character).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Paragraph {
    /// Extra space between lines, in pixels.
    pub line_spacing: f32,
    /// First-line indent of each paragraph.
    pub indent: f32,
    pub margin_left: f32,
    pub margin_right: f32,
    /// Fixed-width text box: words wrap at this width. None = the box grows with the text.
    pub width: Option<f32>,
    pub baseline: Baseline,
    /// Auto-kern off.
    pub no_kern: bool,
}

impl Paragraph {
    pub fn is_default(&self) -> bool {
        *self == Paragraph::default()
    }
}

/// Static, Dynamic (scripts can change its text) or Input (the viewer types into it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextKind {
    #[default]
    Static,
    Dynamic,
    Input,
}

/// Text field options: kind, input limits, border, link.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct TextField {
    pub kind: TextKind,
    /// Input text: most characters accepted (0 = no limit).
    pub max_chars: u32,
    /// Dynamic/Input text the viewer can select.
    pub selectable: bool,
    /// Draw a border and background around the box.
    pub border: bool,
    /// Input text shows bullets instead of the characters.
    pub password: bool,
    /// Clicking the text opens this address (published movies).
    pub link: Option<String>,
}

impl TextField {
    pub fn is_default(&self) -> bool {
        *self == TextField::default()
    }
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
    text_path_styled_para(text, size, family, face, weight, italic, align, letter_spacing, &Paragraph::default())
}

/// `text_path_styled` with paragraph options.
#[allow(clippy::too_many_arguments)]
pub fn text_path_styled_para(text: &str, size: f32, family: Option<&str>, face: FontFace, weight: u16, italic: bool, align: Align, letter_spacing: f32, para: &Paragraph) -> Path {
    let (path, have_w, have_i, ascent) = match family.and_then(|fam| styled_font(fam, weight, italic)) {
        Some((f, w, i)) => (outline_para(&*f, text, size, align, letter_spacing, para), w, i, f.as_scaled(size.max(1.0)).ascent()),
        None => {
            let base_w = if face == FontFace::Sans { 300 } else { 400 };
            (outline_para(font(face), text, size, align, letter_spacing, para), base_w, false, font(face).as_scaled(size.max(1.0)).ascent())
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

/// A line before alignment: glyphs (relative x), width without trailing
/// spaces, last line of its paragraph, first line of its paragraph.
type RawLine = (Vec<(char, ab_glyph::GlyphId, f32)>, f32, bool, bool);

/// One laid-out line: glyphs with their pen x, the line's baseline.
struct Line {
    glyphs: Vec<(char, ab_glyph::GlyphId, f32)>,
    baseline: f32,
}

/// Lay text out: paragraphs on '\n', words wrapping in a fixed-width box,
/// indent, margins, line spacing, alignment (justify), baseline shift and
/// kerning. Returns the lines and the glyph scale font size used.
fn layout<F: Font>(font: &F, text: &str, size: f32, align: Align, letter_spacing: f32, para: &Paragraph) -> (Vec<Line>, f32) {
    let size = size.max(1.0);
    let full = font.as_scaled(size);
    let (gsize, shift) = match para.baseline {
        Baseline::Normal => (size, 0.0),
        Baseline::Superscript => (size * 0.62, -size * 0.38),
        Baseline::Subscript => (size * 0.62, size * 0.12),
    };
    let f = font.as_scaled(gsize);
    let line_h = full.height() + full.line_gap() + para.line_spacing;
    let kern = |a: Option<ab_glyph::GlyphId>, b: ab_glyph::GlyphId| if para.no_kern { 0.0 } else { a.map(|a| f.kern(a, b)).unwrap_or(0.0) };
    let box_w = para.width.map(|w| (w - para.margin_left - para.margin_right).max(gsize));
    // (glyphs relative to the line start, width without trailing spaces, last line of its paragraph, first line of its paragraph)
    let mut raw: Vec<RawLine> = vec![];
    for paragraph in text.split('\n') {
        let chars: Vec<char> = paragraph.chars().collect();
        let mut start = 0;
        let mut first = true;
        loop {
            let avail = box_w.map(|w| w - if first { para.indent } else { 0.0 });
            let mut x = 0.0;
            let mut prev = None;
            let mut line = vec![];
            let mut last_space: Option<usize> = None;
            let mut k = start;
            let mut broke = false;
            while k < chars.len() {
                let ch = chars[k];
                let id = f.glyph_id(ch);
                let gx = x + kern(prev, id);
                let adv = f.h_advance(id) + letter_spacing;
                if let Some(a) = avail {
                    if gx + adv > a && !ch.is_whitespace() && !line.is_empty() {
                        broke = true;
                        break;
                    }
                }
                if ch.is_whitespace() {
                    last_space = Some(line.len());
                }
                line.push((ch, id, gx));
                x = gx + adv;
                prev = Some(id);
                k += 1;
            }
            if broke {
                // back up to the last space when there is one on the line
                if let Some(sp) = last_space.filter(|&sp| sp > 0) {
                    let keep = sp;
                    k = start + keep + 1;
                    line.truncate(keep);
                }
            }
            let width = line.iter().rev().find(|g| !g.0.is_whitespace()).map(|g| g.2 + f.h_advance(g.1)).unwrap_or(0.0);
            let done = k >= chars.len();
            raw.push((line, width, done, first));
            if done {
                break;
            }
            start = k;
            // spaces at the start of a wrapped line are dropped
            while start < chars.len() && chars[start] == ' ' {
                start += 1;
            }
            first = false;
        }
    }
    let content_w = box_w.unwrap_or_else(|| raw.iter().map(|l| l.1 + if l.3 { para.indent } else { 0.0 }).fold(0.0, f32::max));
    let mut lines = vec![];
    for (li, (glyphs, width, last, first)) in raw.into_iter().enumerate() {
        let ind = if first { para.indent } else { 0.0 };
        let room = content_w - ind - width;
        let x0 = para.margin_left
            + ind
            + match align {
                Align::Left | Align::Justify => 0.0,
                Align::Center => room / 2.0,
                Align::Right => room,
            };
        let spaces = glyphs.iter().filter(|g| g.0 == ' ').count();
        let extra = if align == Align::Justify && box_w.is_some() && !last && spaces > 0 { room.max(0.0) / spaces as f32 } else { 0.0 };
        let mut add = 0.0;
        let glyphs = glyphs
            .into_iter()
            .map(|(ch, id, x)| {
                let g = (ch, id, x0 + x + add);
                if ch == ' ' {
                    add += extra;
                }
                g
            })
            .collect();
        lines.push(Line { glyphs, baseline: full.ascent() + li as f32 * line_h + shift });
    }
    (lines, gsize)
}

fn outline_with<F: Font>(font: &F, text: &str, size: f32, align: Align, letter_spacing: f32) -> Path {
    outline_para(font, text, size, align, letter_spacing, &Paragraph::default())
}

fn outline_para<F: Font>(font: &F, text: &str, size: f32, align: Align, letter_spacing: f32, para: &Paragraph) -> Path {
    let (lines, gsize) = layout(font, text, size, align, letter_spacing, para);
    let f = font.as_scaled(gsize);
    let scale = f.scale_factor();
    let mut segs = Vec::new();
    for line in &lines {
        let baseline = line.baseline;
        for &(_, id, x) in &line.glyphs {
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
        }
    }
    Path { segs }
}

/// Outline with paragraph options (TextEl::outline).
#[allow(clippy::too_many_arguments)]
pub fn text_path_para(text: &str, size: f32, family: Option<&str>, face: FontFace, align: Align, letter_spacing: f32, para: &Paragraph) -> Path {
    match family.and_then(family_font) {
        Some(f) => outline_para(&*f, text, size, align, letter_spacing, para),
        None => outline_para(font(face), text, size, align, letter_spacing, para),
    }
}

/// The text box size (width × height) for a layout: the fixed width or the
/// widest line plus margins, by the line count.
pub fn box_size(text: &str, size: f32, family: Option<&str>, face: FontFace, letter_spacing: f32, para: &Paragraph) -> (f32, f32) {
    fn go<F: Font>(font: &F, text: &str, size: f32, ls: f32, para: &Paragraph) -> (f32, f32) {
        let (lines, gsize) = layout(font, text, size, Align::Left, ls, para);
        let f = font.as_scaled(gsize);
        let full = font.as_scaled(size.max(1.0));
        let w = para.width.unwrap_or_else(|| lines.iter().map(|l| l.glyphs.last().map(|g| g.2 + f.h_advance(g.1)).unwrap_or(0.0)).fold(0.0, f32::max) + para.margin_right);
        let h = lines.len().max(1) as f32 * (full.height() + full.line_gap() + para.line_spacing) - para.line_spacing;
        (w, h)
    }
    match family.and_then(family_font) {
        Some(f) => go(&*f, text, size, letter_spacing, para),
        None => go(font(face), text, size, letter_spacing, para),
    }
}

/// Top-left position of every visible character (Break Apart: one text
/// object per letter), same layout as `text_path`.
pub fn letter_positions(text: &str, size: f32, family: Option<&str>, face: FontFace, align: Align, letter_spacing: f32) -> Vec<(char, Pt)> {
    letter_positions_para(text, size, family, face, align, letter_spacing, &Paragraph::default())
}

/// `letter_positions` with paragraph options.
pub fn letter_positions_para(text: &str, size: f32, family: Option<&str>, face: FontFace, align: Align, letter_spacing: f32, para: &Paragraph) -> Vec<(char, Pt)> {
    match family.and_then(family_font) {
        Some(f) => letters_with(&*f, text, size, align, letter_spacing, para),
        None => letters_with(font(face), text, size, align, letter_spacing, para),
    }
}

fn letters_with<F: Font>(font: &F, text: &str, size: f32, align: Align, letter_spacing: f32, para: &Paragraph) -> Vec<(char, Pt)> {
    let (lines, gsize) = layout(font, text, size, align, letter_spacing, para);
    let asc = font.as_scaled(gsize).ascent();
    lines.iter().flat_map(|l| l.glyphs.iter().filter(|g| !g.0.is_whitespace()).map(move |&(ch, _, x)| (ch, pt(x, l.baseline - asc)))).collect()
}
