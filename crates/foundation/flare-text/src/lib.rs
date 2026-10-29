//! Text to vector outlines.
//!
//! The text tool turns what the user types into ordinary shapes, so everything downstream
//! (drawing, hit testing, export) needs no idea that text exists. This crate does the shaping
//! (with `parley`) and reads the glyph outlines (with `skrifa`). A [`TextEngine`] holds the
//! font database, which is slow to build the first time (it scans the system's fonts), so one
//! engine should be created off the UI thread and reused.
#![forbid(unsafe_code)]

use flare_core::DocumentPoint;
use flare_document::{MAX_SEGMENTS, PathSegment};
use parley::style::{FontFamily, FontStyle, FontWeight, StyleProperty};
use parley::{Alignment, AlignmentOptions, FontContext, LayoutContext, PositionedLayoutItem};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::raw::types::F2Dot14;
use skrifa::{FontRef, GlyphId, MetadataProvider};
use thiserror::Error;

/// Most characters one text object may hold.
pub const MAX_CHARS: usize = 10_000;
/// Smallest and largest font size, in stage units.
pub const MIN_SIZE: f32 = 1.0;
pub const MAX_SIZE: f32 = 2_048.0;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TextError {
    #[error("there is no text to draw")]
    Empty,
    #[error("text is limited to {MAX_CHARS} characters")]
    TooLong,
    #[error("the font size must be between {MIN_SIZE} and {MAX_SIZE}")]
    BadSize,
    #[error("the text has more outline segments than a shape can hold")]
    TooComplex,
    #[error("the text has no glyphs in the chosen fonts")]
    NoGlyphs,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextStyle {
    /// A CSS-style family list, such as `"Helvetica, Arial, sans-serif"`.
    pub family: String,
    pub size: f32,
    /// Heavy strokes, using the family's bold face when it has one.
    pub bold: bool,
    /// Slanted letters, using the family's italic face when it has one.
    pub italic: bool,
}

/// The outlines of a text object, with their top-left corner at the origin: x grows right, y
/// grows down, and the text starts at the corner of its first line.
#[derive(Debug, Clone, PartialEq)]
pub struct TextOutline {
    pub path: Vec<PathSegment>,
    pub width: f32,
    pub height: f32,
}

pub struct TextEngine {
    fonts: FontContext,
    layouts: LayoutContext<()>,
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TextEngine {
    /// Builds the font database, discovering the system's fonts. This can take a while.
    pub fn new() -> Self {
        Self {
            fonts: FontContext::new(),
            layouts: LayoutContext::new(),
        }
    }

    /// Makes the fonts in `data` available by their family names, in addition to the system's.
    /// Returns the family names that were added.
    pub fn register_font(&mut self, data: Vec<u8>) -> Vec<String> {
        let registered = self
            .fonts
            .collection
            .register_fonts(parley::fontique::Blob::from(data), None);
        registered
            .into_iter()
            .filter_map(|(id, _)| self.fonts.collection.family_name(id).map(str::to_owned))
            .collect()
    }

    /// Lays `text` out in one block (newlines start new lines) and returns its outlines.
    pub fn outline(&mut self, text: &str, style: &TextStyle) -> Result<TextOutline, TextError> {
        if text.trim().is_empty() {
            return Err(TextError::Empty);
        }
        if text.chars().count() > MAX_CHARS {
            return Err(TextError::TooLong);
        }
        if !style.size.is_finite() || !(MIN_SIZE..=MAX_SIZE).contains(&style.size) {
            return Err(TextError::BadSize);
        }
        let mut builder = self
            .layouts
            .ranged_builder(&mut self.fonts, text, 1.0, true);
        builder.push_default(StyleProperty::FontSize(style.size));
        builder.push_default(StyleProperty::FontFamily(FontFamily::Source(
            style.family.as_str().into(),
        )));
        if style.bold {
            builder.push_default(StyleProperty::FontWeight(FontWeight::BOLD));
        }
        if style.italic {
            builder.push_default(StyleProperty::FontStyle(FontStyle::Italic));
        }
        let mut layout = builder.build(text);
        layout.break_all_lines(None);
        layout.align(Alignment::Start, AlignmentOptions::default());

        let mut path = Vec::new();
        let mut glyphs = 0_usize;
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                let font = run.run().font();
                let Ok(font_ref) = FontRef::from_index(font.data.as_ref(), font.index) else {
                    continue;
                };
                let coords: Vec<F2Dot14> = run
                    .run()
                    .normalized_coords()
                    .iter()
                    .map(|c| F2Dot14::from_bits(c.to_bits()))
                    .collect();
                let location = LocationRef::new(&coords);
                let outlines = font_ref.outline_glyphs();
                for glyph in run.positioned_glyphs() {
                    let Some(outline) = outlines.get(GlyphId::new(glyph.id)) else {
                        continue;
                    };
                    let mut pen = Pen {
                        out: &mut path,
                        at: (glyph.x, glyph.y),
                        bad: false,
                    };
                    let settings =
                        DrawSettings::unhinted(Size::new(run.run().font_size()), location);
                    if outline.draw(settings, &mut pen).is_err() || pen.bad {
                        continue;
                    }
                    glyphs += 1;
                    if path.len() > MAX_SEGMENTS {
                        return Err(TextError::TooComplex);
                    }
                }
            }
        }
        if path.is_empty() {
            return Err(if glyphs == 0 {
                TextError::NoGlyphs
            } else {
                TextError::Empty
            });
        }
        Ok(TextOutline {
            path,
            width: layout.width(),
            height: layout.height(),
        })
    }
}

/// Collects a glyph's outline as path segments, moved to where the glyph sits. Font outlines
/// are y-up with the baseline at zero; the stage is y-down, so y is flipped.
struct Pen<'a> {
    out: &'a mut Vec<PathSegment>,
    at: (f32, f32),
    bad: bool,
}

impl Pen<'_> {
    fn point(&mut self, x: f32, y: f32) -> Option<DocumentPoint> {
        let p = DocumentPoint::new(f64::from(self.at.0 + x), f64::from(self.at.1 - y)).ok();
        if p.is_none() {
            self.bad = true;
        }
        p
    }
}

impl OutlinePen for Pen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        if let Some(p) = self.point(x, y) {
            self.out.push(PathSegment::MoveTo(p));
        }
    }

    fn line_to(&mut self, x: f32, y: f32) {
        if let Some(p) = self.point(x, y) {
            self.out.push(PathSegment::LineTo(p));
        }
    }

    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        // A shape's curves are cubic. The control points of the equal cubic sit two thirds of
        // the way from each end to the quadratic's control point.
        let Some(
            &PathSegment::MoveTo(from)
            | &PathSegment::LineTo(from)
            | &PathSegment::CubicTo(_, _, from),
        ) = self.out.last()
        else {
            return;
        };
        let (Some(c), Some(to)) = (self.point(cx, cy), self.point(x, y)) else {
            return;
        };
        let (f, c, t) = (from.as_vec(), c.as_vec(), to.as_vec());
        let one = f + (c - f) * (2.0 / 3.0);
        let two = t + (c - t) * (2.0 / 3.0);
        if let (Ok(a), Ok(b)) = (
            DocumentPoint::new(one.x, one.y),
            DocumentPoint::new(two.x, two.y),
        ) {
            self.out.push(PathSegment::CubicTo(a, b, to));
        }
    }

    fn curve_to(&mut self, ax: f32, ay: f32, bx: f32, by: f32, x: f32, y: f32) {
        if let (Some(a), Some(b), Some(to)) =
            (self.point(ax, ay), self.point(bx, by), self.point(x, y))
        {
            self.out.push(PathSegment::CubicTo(a, b, to));
        }
    }

    fn close(&mut self) {
        self.out.push(PathSegment::Close);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    /// An engine that knows the Hack font by name, so tests do not depend on what the machine
    /// has installed.
    fn engine() -> TextEngine {
        let mut engine = TextEngine::new();
        let names = engine.register_font(epaint_default_fonts::HACK_REGULAR.to_vec());
        assert!(names.iter().any(|n| n == "Hack"), "{names:?}");
        engine
    }

    fn style(size: f32) -> TextStyle {
        TextStyle {
            bold: false,
            italic: false,
            family: "Hack".into(),
            size,
        }
    }

    fn bounds(path: &[PathSegment]) -> (f64, f64, f64, f64) {
        let mut b = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        let mut add = |p: &DocumentPoint| {
            let v = p.as_vec();
            b = (b.0.min(v.x), b.1.min(v.y), b.2.max(v.x), b.3.max(v.y));
        };
        for s in path {
            match s {
                PathSegment::MoveTo(p) | PathSegment::LineTo(p) => add(p),
                PathSegment::CubicTo(a, c, p) => {
                    add(a);
                    add(c);
                    add(p);
                }
                PathSegment::Close => {}
            }
        }
        b
    }

    #[test]
    fn letters_become_closed_outlines_inside_their_line() {
        let out = engine().outline("Hi", &style(40.0)).expect("outline");
        assert!(matches!(out.path[0], PathSegment::MoveTo(_)));
        assert!(out.path.iter().any(|s| matches!(s, PathSegment::Close)));
        let (min_x, min_y, max_x, max_y) = bounds(&out.path);
        assert!(
            min_x >= -1.0 && max_x <= f64::from(out.width) + 1.0,
            "{min_x}..{max_x} in {}",
            out.width
        );
        assert!(
            min_y >= -1.0 && max_y <= f64::from(out.height) + 1.0,
            "{min_y}..{max_y} in {}",
            out.height
        );
        assert!(
            max_y - min_y > 20.0,
            "a 40 unit capital is tall: {}",
            max_y - min_y
        );
    }

    #[test]
    fn text_scales_with_the_size_and_grows_with_its_length() {
        let mut e = engine();
        let small = e.outline("ab", &style(20.0)).expect("small");
        let big = e.outline("ab", &style(40.0)).expect("big");
        assert!(
            (big.width / small.width - 2.0).abs() < 0.05,
            "{} vs {}",
            big.width,
            small.width
        );
        let longer = e.outline("abab", &style(20.0)).expect("longer");
        assert!(
            (longer.width / small.width - 2.0).abs() < 0.05,
            "a monospace font"
        );
    }

    #[test]
    fn newlines_start_new_lines() {
        let mut e = engine();
        let one = e.outline("ab", &style(30.0)).expect("one line");
        let two = e.outline("ab\nab", &style(30.0)).expect("two lines");
        assert!(
            (two.height / one.height - 2.0).abs() < 0.1,
            "{} vs {}",
            two.height,
            one.height
        );
        assert!(
            (two.width - one.width).abs() < 1.0,
            "the width is that of the widest line"
        );
    }

    #[test]
    fn a_glyph_with_a_hole_keeps_it_as_a_second_contour() {
        let out = engine().outline("o", &style(60.0)).expect("outline");
        let contours = out
            .path
            .iter()
            .filter(|s| matches!(s, PathSegment::MoveTo(_)))
            .count();
        assert_eq!(contours, 2, "outer and inner contour of an o");
        assert!(
            out.path
                .iter()
                .any(|s| matches!(s, PathSegment::CubicTo(..))),
            "curves"
        );
    }

    #[test]
    fn unusable_requests_are_refused_with_reasons() {
        let mut e = engine();
        assert_eq!(
            e.outline("", &style(20.0)).expect_err("refused"),
            TextError::Empty
        );
        assert_eq!(
            e.outline("  \n ", &style(20.0)).expect_err("refused"),
            TextError::Empty
        );
        for size in [0.0, 0.5, MAX_SIZE + 1.0, f32::NAN, f32::INFINITY, -4.0] {
            assert_eq!(
                e.outline("a", &style(size)).expect_err("refused"),
                TextError::BadSize,
                "{size}"
            );
        }
        let long = "a".repeat(MAX_CHARS + 1);
        assert_eq!(
            e.outline(&long, &style(10.0)).expect_err("refused"),
            TextError::TooLong
        );
        assert!(
            e.outline(&"a".repeat(MAX_CHARS), &style(10.0)).is_ok(),
            "the limit itself works"
        );
    }

    #[test]
    fn an_unknown_family_falls_back_instead_of_failing() {
        let mut e = engine();
        let style = TextStyle {
            bold: false,
            italic: false,
            family: "No Such Font, Hack".into(),
            size: 30.0,
        };
        assert!(e.outline("a", &style).is_ok());
    }

    #[test]
    fn bold_and_italic_are_accepted_and_still_give_outlines() {
        let mut engine = engine();
        for (bold, italic) in [(true, false), (false, true), (true, true)] {
            let styled = TextStyle {
                bold,
                italic,
                ..style(32.0)
            };
            let outline = engine.outline("Flare", &styled).expect("outline");
            assert!(!outline.path.is_empty() && outline.width > 0.0);
        }
    }
}
