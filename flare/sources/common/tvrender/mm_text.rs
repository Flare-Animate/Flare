//! Classic text: where a text field's characters go, its glyphs as a shape
//! (which the renderer, hit tests and SWF export use), and editing its runs.

use std::ops::Range;
use std::sync::Arc;

use skrifa::charmap::Charmap;
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::GlyphMetrics;
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, MetadataProvider, OutlineGlyphCollection};

use crate::doc::*;
use crate::{draw, fonts};

/// Space between a text box's edges and its text, as in Animate.
pub const GUTTER: f32 = 2.0;

/// Where a text field's characters go, in its own coordinates: the top left
/// corner of its box is the origin.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The box, gutters included.
    pub width: f32,
    pub height: f32,
    pub lines: Vec<Line>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    /// The index of its first character among the text's characters, newlines included.
    pub start: usize,
    pub baseline: f32,
    /// How far its tallest font reaches above and below the baseline.
    pub ascent: f32,
    pub descent: f32,
    pub glyphs: Vec<Glyph>,
    /// Where a character after its last would go.
    pub end_x: f32,
}

/// A character of run `run`, its advance starting at `x` on the line's baseline.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub run: usize,
    pub c: char,
    pub x: f32,
}

impl Layout {
    /// The line the caret before character `index` shows on: where a line wraps,
    /// the next one's start.
    pub fn line_of(&self, index: usize) -> usize {
        self.lines.iter().rposition(|l| l.start <= index).unwrap_or(0)
    }

    /// Where the caret before character `index` goes on line `l`, clamped to the line.
    pub fn x_of(&self, l: usize, index: usize) -> f32 {
        let Some(line) = self.lines.get(l) else { return GUTTER };
        line.glyphs.get(index.saturating_sub(line.start)).map_or(line.end_x, |g| g.x)
    }

    /// The last caret position on line `l`: before a wrapped line's last character
    /// (its hanging space), since after it is the next line's start.
    pub fn line_end(&self, l: usize) -> usize {
        let Some(line) = self.lines.get(l) else { return 0 };
        let end = line.start + line.glyphs.len();
        let wraps = self.lines.get(l + 1).is_some_and(|next| next.start == end);
        if wraps { end.saturating_sub(1).max(line.start) } else { end }
    }

    /// The caret position nearest `(x, y)`, in the text's own coordinates.
    pub fn index_at(&self, (x, y): Point) -> usize {
        let Some(l) = self.lines.iter().rposition(|l| y >= l.baseline - l.ascent).or((!self.lines.is_empty()).then_some(0)) else {
            return 0;
        };
        let line = &self.lines[l];
        let xs: Vec<f32> = line.glyphs.iter().map(|g| g.x).chain([line.end_x]).collect();
        let k = xs.windows(2).position(|w| x < (w[0] + w[1]) / 2.0).unwrap_or(line.glyphs.len());
        (line.start + k).min(self.line_end(l))
    }
}

/// Lays the text out with its fonts.
// ponytail: no kerning, no fallback fonts for characters a face lacks (they draw its
// missing-glyph box), and no faux bold or italic for families without those faces.
pub fn layout(text: &Text) -> Layout {
    let files = font_files(text);
    layout_with(text, &run_fonts(text, &files))
}

/// The text's glyphs as filled outlines, one edge per glyph, at the text's matrix.
pub fn to_shape(text: &Text) -> Shape {
    let files = font_files(text);
    let fonts = run_fonts(text, &files);
    let mut shape = Shape { matrix: text.matrix, fills: Vec::new(), strokes: Vec::new(), edges: Vec::new(), extra: Extra::default() };
    for line in layout_with(text, &fonts).lines {
        for g in line.glyphs {
            let font = &fonts[g.run];
            let Some((charmap, _, outlines)) = &font.glyphs else { continue };
            let Some(glyph) = outlines.get(charmap.map(g.c).unwrap_or_default()) else { continue };
            let mut pen = Pen { origin: (g.x, line.baseline), paths: Vec::new() };
            if glyph.draw(DrawSettings::unhinted(Size::new(font.size), LocationRef::default()), &mut pen).is_err() {
                continue;
            }
            pen.close();
            pen.paths.retain(|p| !p.segs.is_empty());
            if pen.paths.is_empty() {
                continue;
            }
            let paint = Paint::Solid(text.runs[g.run].color);
            let fill = match shape.fills.iter().position(|f| *f == paint) {
                Some(i) => i + 1,
                None => {
                    shape.fills.push(paint);
                    shape.fills.len()
                }
            } as u32;
            // A glyph's outer contours run one way round and its holes the other, so
            // the glyph lies on the same side of all its edges: the right if it runs clockwise.
            let right = twice_area(&pen.paths) > 0.0;
            let (fill0, fill1) = if right { (0, fill) } else { (fill, 0) };
            shape.edges.push(Edge { fill0, fill1, stroke: 0, paths: pen.paths });
        }
    }
    shape
}

/// The text box as a filled rectangle at the text's matrix: text is hit and
/// selected by its box, as in Animate.
pub fn box_shape(text: &Text) -> Shape {
    let l = layout(text);
    let mut shape = draw::rectangle([(0.0, 0.0), (l.width, l.height)], 0.0, None, Some(Paint::Solid([0.0; 4])));
    shape.matrix = text.matrix;
    shape
}

/// The text's characters, newlines included.
pub fn string(text: &Text) -> String {
    text.runs.iter().map(|r| r.text.as_str()).collect()
}

/// Replaces characters `range` (character indices) with `with`, which takes the
/// style of the character before the range (or of the first, at the start).
/// `style` styles text that has no runs. Runs left empty go, but the last one
/// stays to keep its style.
pub fn replace(text: &mut Text, range: Range<usize>, with: &str, style: &TextRun) {
    let mut chars: Vec<(char, usize)> = text.runs.iter().enumerate().flat_map(|(i, r)| r.text.chars().map(move |c| (c, i))).collect();
    let mut styles = std::mem::take(&mut text.runs);
    if styles.is_empty() {
        styles.push(style.clone());
    }
    styles.iter_mut().for_each(|s| s.text.clear());
    let end = range.end.min(chars.len());
    let start = range.start.min(end);
    let new_style = chars.get(start.saturating_sub(1)).map_or(0, |&(_, s)| s);
    chars.splice(start..end, with.chars().map(|c| (c, new_style)));
    let mut last = None;
    for (c, s) in chars {
        if last != Some(s) {
            text.runs.push(styles[s].clone());
            last = Some(s);
        }
        if let Some(run) = text.runs.last_mut() {
            run.text.push(c);
        }
    }
    if text.runs.is_empty() {
        text.runs.push(styles.swap_remove(new_style));
    }
}

type FontFile = Option<(Arc<Vec<u8>>, u32)>;

/// Each run's font file and face index; None when no fonts are installed.
fn font_files(text: &Text) -> Vec<FontFile> {
    text.runs.iter().map(|run| {
        let face = fonts::resolve(&run.face, run.bold, run.italic)?;
        Some((fonts::load(face)?, face.index))
    }).collect()
}

fn run_fonts<'a>(text: &Text, files: &'a [FontFile]) -> Vec<RunFont<'a>> {
    text.runs.iter().zip(files).map(|(run, file)| RunFont::new(run, file.as_ref())).collect()
}

/// A run's face at the run's size. Without one (no fonts installed) every
/// character is half an em wide and draws nothing.
struct RunFont<'a> {
    glyphs: Option<(Charmap<'a>, GlyphMetrics<'a>, OutlineGlyphCollection<'a>)>,
    size: f32,
    ascent: f32,
    descent: f32,
}

impl<'a> RunFont<'a> {
    fn new(run: &TextRun, file: Option<&'a (Arc<Vec<u8>>, u32)>) -> Self {
        let size = Size::new(run.size);
        match file.and_then(|(data, index)| FontRef::from_index(data.as_slice(), *index).ok()) {
            Some(font) => {
                let m = font.metrics(size, LocationRef::default());
                let glyphs = (font.charmap(), font.glyph_metrics(size, LocationRef::default()), font.outline_glyphs());
                RunFont { glyphs: Some(glyphs), size: run.size, ascent: m.ascent, descent: -m.descent }
            }
            // Arial's proportions.
            None => RunFont { glyphs: None, size: run.size, ascent: 0.905 * run.size, descent: 0.212 * run.size },
        }
    }

    fn advance(&self, c: char) -> f32 {
        match &self.glyphs {
            Some((charmap, metrics, _)) => metrics.advance_width(charmap.map(c).unwrap_or_default()).unwrap_or(0.0),
            None => self.size / 2.0,
        }
    }
}

struct Char {
    run: usize,
    c: char,
    advance: f32,
}

fn layout_with(text: &Text, fonts: &[RunFont]) -> Layout {
    if text.runs.is_empty() {
        let width = text.width.unwrap_or(2.0 * GUTTER);
        return Layout { width, height: 2.0 * GUTTER, lines: Vec::new() };
    }
    // Paragraphs: the run whose font sizes it if it's empty (the run of the newline
    // before it), where it starts, and its characters.
    let (mut paragraphs, mut chars, mut first_run, mut start) = (Vec::new(), Vec::new(), 0, 0);
    let mut index = 0;
    for (run, r) in text.runs.iter().enumerate() {
        for c in r.text.chars() {
            index += 1;
            if c == '\n' {
                paragraphs.push((first_run, start, std::mem::take(&mut chars)));
                (first_run, start) = (run, index);
            } else {
                chars.push(Char { run, c, advance: fonts[run].advance(c) + r.letter_spacing });
            }
        }
    }
    paragraphs.push((first_run, start, chars));

    let wrap = text.width.map(|w| (w - 2.0 * GUTTER).max(0.0));
    // Each line: its paragraph's run, where it starts, its characters, and whether it ends by wrapping.
    let mut lines: Vec<(usize, usize, &[Char], bool)> = Vec::new();
    for (run, start, chars) in &paragraphs {
        let breaks = wrap.map_or_else(|| vec![0..chars.len()], |w| wrap_lines(chars, w));
        let last = breaks.len() - 1;
        lines.extend(breaks.into_iter().enumerate().map(|(i, r)| (*run, start + r.start, &chars[r], i < last)));
    }
    // A wrapped line's trailing spaces hang past the edge: alignment ignores them.
    let content = |chars: &[Char], wrapped: bool| chars.len() - if wrapped { chars.iter().rev().take_while(|c| c.c == ' ').count() } else { 0 };
    let width_of = |chars: &[Char], wrapped| chars[..content(chars, wrapped)].iter().map(|c| c.advance).sum::<f32>();
    let inner = wrap.unwrap_or_else(|| lines.iter().map(|&(_, _, chars, wrapped)| width_of(chars, wrapped)).fold(0.0, f32::max));

    let (mut top, mut bottom, mut out) = (GUTTER, GUTTER, Vec::with_capacity(lines.len()));
    for &(run, start, chars, wrapped) in &lines {
        let mut fonts_used: Vec<&RunFont> = chars.iter().map(|c| &fonts[c.run]).collect();
        if fonts_used.is_empty() {
            fonts_used.push(&fonts[run]);
        }
        let ascent = fonts_used.iter().map(|f| f.ascent).fold(0.0, f32::max);
        let descent = fonts_used.iter().map(|f| f.descent).fold(0.0, f32::max);
        let baseline = top + ascent;
        let end = content(chars, wrapped);
        let extra = inner - width_of(chars, wrapped);
        let spaces = chars[..end].iter().filter(|c| c.c == ' ').count();
        let (mut x, gap) = match text.align {
            TextAlign::Left => (0.0, 0.0),
            TextAlign::Center => (extra / 2.0, 0.0),
            TextAlign::Right => (extra, 0.0),
            // Lines stretch at their spaces, except each paragraph's last.
            TextAlign::Justify if wrapped && spaces > 0 => (0.0, extra / spaces as f32),
            TextAlign::Justify => (0.0, 0.0),
        };
        x += GUTTER;
        let mut glyphs = Vec::with_capacity(chars.len());
        for (i, c) in chars.iter().enumerate() {
            glyphs.push(Glyph { run: c.run, c: c.c, x });
            x += c.advance + if c.c == ' ' && i < end { gap } else { 0.0 };
        }
        out.push(Line { start, baseline, ascent, descent, glyphs, end_x: x });
        bottom = baseline + descent;
        top = bottom + text.line_spacing;
    }
    Layout { width: text.width.unwrap_or(inner + 2.0 * GUTTER), height: bottom + GUTTER, lines: out }
}

/// Breaks a paragraph into lines no wider than `width`: after the last space
/// that fits, or inside a word too wide on its own. Spaces may hang past the edge.
fn wrap_lines(chars: &[Char], width: f32) -> Vec<Range<usize>> {
    let (mut lines, mut start, mut x, mut after_space) = (Vec::new(), 0, 0.0, None);
    for (i, ch) in chars.iter().enumerate() {
        if ch.c != ' ' && i > start && x + ch.advance > width {
            let end = after_space.unwrap_or(i);
            lines.push(start..end);
            x = chars[end..i].iter().map(|c| c.advance).sum();
            (start, after_space) = (end, None);
        }
        x += ch.advance;
        if ch.c == ' ' {
            after_space = Some(i + 1);
        }
    }
    lines.push(start..chars.len());
    lines
}

/// Collects a glyph's contours as closed paths placed at `origin`. Fonts point y up, the stage down.
struct Pen {
    origin: Point,
    paths: Vec<Path>,
}

impl Pen {
    fn at(&self, x: f32, y: f32) -> Point {
        (self.origin.0 + x, self.origin.1 - y)
    }

    fn push(&mut self, seg: Seg) {
        if let Some(path) = self.paths.last_mut() {
            path.segs.push(seg);
        }
    }
}

impl OutlinePen for Pen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close();
        self.paths.push(Path { start: self.at(x, y), segs: Vec::new() });
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.push(Seg::Line(self.at(x, y)));
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.push(Seg::Quad(self.at(cx, cy), self.at(x, y)));
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let Some(path) = self.paths.last() else { return };
        let from = path.segs.last().map_or(path.start, |s| s.end());
        for seg in cubic_to_quads([from, self.at(cx0, cy0), self.at(cx1, cy1), self.at(x, y)]) {
            self.push(seg);
        }
    }

    /// Ends the contour back at its start: XFL edges spell out every segment.
    fn close(&mut self) {
        if let Some(path) = self.paths.last_mut() {
            let end = path.segs.last().map_or(path.start, |s| s.end());
            if end != path.start {
                path.segs.push(Seg::Line(path.start));
            }
        }
    }
}

/// A cubic curve (CFF fonts) as quadratic ones, the shapes' only curves. Each
/// piece becomes the quadratic through its ends whose control point averages
/// the extensions of its two control points; that strays from it by √3/36 of
/// its third difference, which splitting into n pieces cuts by n³. Pieces stay
/// within a hundredth of a pixel.
fn cubic_to_quads([p0, p1, p2, p3]: [Point; 4]) -> impl Iterator<Item = Seg> {
    let third = (p3.0 - 3.0 * (p2.0 - p1.0) - p0.0).hypot(p3.1 - 3.0 * (p2.1 - p1.1) - p0.1);
    let n = (third * 3f32.sqrt() / 36.0 / 0.01).cbrt().ceil().clamp(1.0, 32.0) as u32;
    let at = move |t: f32| {
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        (a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0, a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1)
    };
    let tangent = move |t: f32| {
        let u = 1.0 - t;
        let (a, b, c) = (3.0 * u * u, 6.0 * u * t, 3.0 * t * t);
        let along = |i: fn(Point) -> f32| a * (i(p1) - i(p0)) + b * (i(p2) - i(p1)) + c * (i(p3) - i(p2));
        (along(|p| p.0), along(|p| p.1))
    };
    (0..n).map(move |i| {
        let (t0, t1) = (i as f32 / n as f32, (i + 1) as f32 / n as f32);
        let ((a, da), (b, db), h) = ((at(t0), tangent(t0)), (at(t1), tangent(t1)), (t1 - t0) / 3.0);
        // The piece's own control points, extended to where a quadratic's would be, averaged.
        let control = |i: fn(Point) -> f32| (3.0 * (i(a) + h * i(da) + i(b) - h * i(db)) - i(a) - i(b)) / 4.0;
        Seg::Quad((control(|p| p.0), control(|p| p.1)), b)
    })
}

/// Twice the area `paths` enclose, taking curves by their control points:
/// positive if they run clockwise on screen.
fn twice_area(paths: &[Path]) -> f32 {
    let cross = |a: Point, b: Point| a.0 * b.1 - b.0 * a.1;
    let mut sum = 0.0;
    for path in paths {
        let mut from = path.start;
        for &seg in &path.segs {
            sum += match seg {
                Seg::Line(to) => cross(from, to),
                Seg::Quad(c, to) => cross(from, c) + cross(c, to),
            };
            from = seg.end();
        }
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::render;

    fn run(text: &str, size: f32) -> TextRun {
        let color = [0.0, 0.0, 0.0, 1.0];
        let extra = Extra::default();
        TextRun { text: text.into(), face: "Arial".into(), size, color, bold: false, italic: false, letter_spacing: 0.0, extra }
    }

    fn field(width: Option<f32>, align: TextAlign, runs: Vec<TextRun>) -> Text {
        Text { kind: TextKind::Static, matrix: Matrix::IDENTITY, width, align, line_spacing: 2.0, runs, extra: Extra::default() }
    }

    /// Lays `text` out as when no fonts are installed: every character 0.5 em wide.
    fn monospace(text: &Text) -> Layout {
        let fonts: Vec<RunFont> = text.runs.iter().map(|r| RunFont::new(r, None)).collect();
        layout_with(text, &fonts)
    }

    /// Each line's text, and where its first character starts.
    fn lines(l: &Layout) -> Vec<(String, f32)> {
        l.lines.iter().map(|line| (line.glyphs.iter().map(|g| g.c).collect(), line.glyphs.first().map_or(-1.0, |g| g.x))).collect()
    }

    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn wraps_at_spaces_and_inside_long_words() {
        // 10 px characters 5 px wide: 6 fit the 34 px box between its gutters.
        let l = monospace(&field(Some(34.0), TextAlign::Left, vec![run("aaa bbb ccc", 10.0)]));
        assert_eq!(lines(&l), [("aaa ".into(), 2.0), ("bbb ".into(), 2.0), ("ccc".into(), 2.0)]);
        assert_eq!(l.lines.iter().map(|l| l.start).collect::<Vec<_>>(), [0, 4, 8]);
        // Lines are ascent + descent apart, plus the line spacing.
        let pitch = 9.05 + 2.12 + 2.0;
        assert!(near(l.lines[0].baseline, 2.0 + 9.05) && near(l.lines[2].baseline, 2.0 + 9.05 + 2.0 * pitch), "{l:?}");
        assert!(near(l.width, 34.0) && near(l.height, 2.0 + 3.0 * pitch - 2.0 + 2.0), "{l:?}");

        let l = monospace(&field(Some(34.0), TextAlign::Left, vec![run("abcdefghij", 10.0)]));
        assert_eq!(lines(&l), [("abcdef".into(), 2.0), ("ghij".into(), 2.0)]);
        // Spaces hang: "abcdef " still fits.
        let l = monospace(&field(Some(34.0), TextAlign::Left, vec![run("abcdef   gh", 10.0)]));
        assert_eq!(lines(&l), [("abcdef   ".into(), 2.0), ("gh".into(), 2.0)]);
    }

    #[test]
    fn newlines_and_extending_boxes() {
        let l = monospace(&field(None, TextAlign::Left, vec![run("ab\n\nabcd", 10.0)]));
        assert_eq!(lines(&l), [("ab".into(), 2.0), ("".into(), -1.0), ("abcd".into(), 2.0)]);
        assert_eq!(l.lines.iter().map(|l| l.start).collect::<Vec<_>>(), [0, 3, 4], "newlines count as characters");
        assert!(near(l.width, 24.0), "as wide as the widest line, plus gutters");
        assert!(near(l.lines[1].baseline - l.lines[0].baseline, 13.17), "an empty line keeps its height");

        let empty = monospace(&field(None, TextAlign::Left, vec![run("", 10.0)]));
        assert!(near(empty.width, 4.0) && near(empty.height, 4.0 + 11.17), "{empty:?}");
        assert_eq!(empty.lines.len(), 1, "a line for the caret");
    }

    #[test]
    fn alignment() {
        let x = |align, width| lines(&monospace(&field(width, align, vec![run("a\nbbb", 10.0)])))[0].1;
        assert_eq!(x(TextAlign::Left, None), 2.0);
        assert_eq!(x(TextAlign::Center, None), 7.0, "centered on the widest line");
        assert_eq!(x(TextAlign::Right, None), 12.0);
        assert_eq!(x(TextAlign::Right, Some(54.0)), 47.0, "against the box");

        // Justified: the first line's space takes up the slack; the last line stays put.
        let l = monospace(&field(Some(34.0), TextAlign::Justify, vec![run("aa bb cc dd", 10.0)]));
        assert_eq!(lines(&l), [("aa bb ".into(), 2.0), ("cc dd".into(), 2.0)]);
        let xs: Vec<f32> = l.lines[0].glyphs.iter().map(|g| g.x).collect();
        assert_eq!(xs, [2.0, 7.0, 12.0, 22.0, 27.0, 32.0]);
        assert_eq!(l.lines[1].glyphs[3].x, 17.0);
    }

    #[test]
    fn runs_share_lines() {
        let mut spaced = run("cd", 20.0);
        spaced.letter_spacing = 1.0;
        let l = monospace(&field(None, TextAlign::Left, vec![run("ab", 10.0), spaced]));
        let xs: Vec<(usize, f32)> = l.lines[0].glyphs.iter().map(|g| (g.run, g.x)).collect();
        assert_eq!(xs, [(0, 2.0), (0, 7.0), (1, 12.0), (1, 23.0)]);
        assert!(near(l.lines[0].baseline, 2.0 + 18.1), "the larger run's ascent");
        assert!(near(l.width, 36.0) && near(l.lines[0].end_x, 34.0));
    }

    #[test]
    fn carets_follow_the_layout() {
        // "aaa " wraps before "bbb", then a paragraph break and "cc": characters 5 px wide.
        let l = monospace(&field(Some(34.0), TextAlign::Left, vec![run("aaa bbb\ncc", 10.0)]));
        assert_eq!(lines(&l), [("aaa ".into(), 2.0), ("bbb".into(), 2.0), ("cc".into(), 2.0)]);
        let caret = |i| (l.line_of(i), l.x_of(l.line_of(i), i));
        assert_eq!(caret(0), (0, 2.0));
        assert_eq!(caret(3), (0, 17.0));
        assert_eq!(caret(4), (1, 2.0), "the wrap goes to the next line");
        assert_eq!(caret(7), (1, 17.0), "before the newline");
        assert_eq!(caret(8), (2, 2.0));
        assert_eq!(caret(10), (2, 12.0));

        let y = |line: usize| l.lines[line].baseline;
        assert_eq!(l.index_at((0.0, y(0))), 0);
        assert_eq!(l.index_at((8.0, y(0))), 1, "nearest boundary");
        assert_eq!(l.index_at((99.0, y(0))), 3, "past a wrapped line's end: before its hanging space");
        assert_eq!(l.index_at((99.0, y(1))), 7);
        assert_eq!(l.index_at((99.0, 999.0)), 10, "below the last line");
        assert_eq!(l.index_at((0.0, -9.0)), 0, "above the first");
        assert_eq!((l.line_end(0), l.line_end(1), l.line_end(2)), (3, 7, 10));
    }

    #[test]
    fn replacing_characters_keeps_styles() {
        let mut big = run("CD", 20.0);
        big.color = [1.0, 0.0, 0.0, 1.0];
        let style = run("", 30.0);
        let mut t = field(None, TextAlign::Left, vec![run("ab", 10.0), big]);
        let runs = |t: &Text| t.runs.iter().map(|r| (r.text.clone(), r.size)).collect::<Vec<_>>();

        replace(&mut t, 2..2, "x", &style);
        assert_eq!(runs(&t), [("abx".into(), 10.0), ("CD".into(), 20.0)], "typing takes the style before the caret");
        replace(&mut t, 0..0, "y", &style);
        assert_eq!(runs(&t), [("yabx".into(), 10.0), ("CD".into(), 20.0)], "or the first character's at the start");
        replace(&mut t, 3..5, "\n", &style);
        assert_eq!(runs(&t), [("yab\n".into(), 10.0), ("D".into(), 20.0)], "across runs");
        assert_eq!(string(&t), "yab\nD");
        replace(&mut t, 0..99, "", &style);
        assert_eq!(runs(&t), [("".into(), 10.0)], "emptied, the style stays");

        let mut blank = field(None, TextAlign::Left, Vec::new());
        replace(&mut blank, 0..0, "hi", &style);
        assert_eq!(runs(&blank), [("hi".into(), 30.0)], "no runs: the style given");
    }

    #[test]
    fn cubics_become_close_quadratics() {
        let cubic = [(0.0, 0.0), (30.0, -60.0), (90.0, 60.0), (100.0, 0.0)];
        let quads: Vec<Seg> = cubic_to_quads(cubic).collect();
        assert!(quads.len() > 1, "a wavy curve takes several pieces");
        let n = quads.len() as f32;
        let bezier = |[a, b, c, d]: [Point; 4], t: f32| {
            let u = 1.0 - t;
            let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
            (w[0] * a.0 + w[1] * b.0 + w[2] * c.0 + w[3] * d.0, w[0] * a.1 + w[1] * b.1 + w[2] * c.1 + w[3] * d.1)
        };
        let mut from = cubic[0];
        for (i, seg) in quads.into_iter().enumerate() {
            let Seg::Quad(c, to) = seg else { panic!("{seg:?}") };
            for k in 0..=16 {
                let s = k as f32 / 16.0;
                let u = 1.0 - s;
                let q = (u * u * from.0 + 2.0 * u * s * c.0 + s * s * to.0, u * u * from.1 + 2.0 * u * s * c.1 + s * s * to.1);
                let p = bezier(cubic, (i as f32 + s) / n);
                assert!((p.0 - q.0).hypot(p.1 - q.1) < 0.02, "piece {i} at {s}: {q:?} vs {p:?}");
            }
            from = to;
        }
        assert!(from == cubic[3], "ends where the cubic does");
        assert_eq!(cubic_to_quads([(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]).count(), 1);
    }

    #[test]
    fn renders_glyph_outlines() {
        if !fonts::system().iter().any(|f| f.postscript == "ArialMT") {
            eprintln!("Arial isn't installed: skipped");
            return;
        }
        let mut text = field(None, TextAlign::Left, vec![run("O", 100.0)]);
        text.runs[0].color = [1.0, 0.0, 0.0, 1.0];
        text.matrix = Matrix { tx: 10.0, ty: 10.0, ..Matrix::IDENTITY };
        let shape = to_shape(&text);
        assert_eq!(shape.edges.len(), 1, "one glyph");
        assert!(shape.edges.iter().all(|e| (e.fill0 == 0) != (e.fill1 == 0)), "filled on one side");
        let points: Vec<Point> = shape.edges[0].paths.iter().flat_map(|p| p.segs.iter().map(|s| s.end())).collect();
        let (x0, x1) = points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.0), hi.max(p.0)));
        let (y0, y1) = points.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.1), hi.max(p.1)));
        assert!(x1 - x0 > 60.0 && y1 - y0 > 60.0, "a 100 px O: {x0} {y0} {x1} {y1}");

        let mut doc = Document::new(140.0, 140.0, 24.0);
        doc.scenes[0].layers[0].frames[0].elements.push(Element::Text(text));
        let (pixmap, skipped) = render(&doc, 0, 1.0).unwrap();
        assert!(skipped.is_empty(), "{skipped:?}");
        let rgb = |x: f32, y: f32| {
            let c = pixmap.pixel((x + 10.0) as u32, (y + 10.0) as u32).unwrap().demultiply();
            [c.red(), c.green(), c.blue()]
        };
        let mid = (y0 + y1) / 2.0;
        assert_eq!(rgb(x0 + 0.04 * (x1 - x0), mid), [255, 0, 0], "the O's ring");
        assert_eq!(rgb((x0 + x1) / 2.0, mid), [255, 255, 255], "its hole");
        assert_eq!(rgb(x0 - 3.0, mid), [255, 255, 255], "beside it");
    }
}
