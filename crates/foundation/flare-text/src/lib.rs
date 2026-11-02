//! Text to vector outlines.
//!
//! The text tool turns what the user types into ordinary shapes, so everything downstream
//! (drawing, hit testing, export) needs no idea that text exists. This crate does the shaping
//! (with `parley`) and reads the glyph outlines (with `skrifa`). A [`TextEngine`] holds the
//! font database, which is slow to build the first time (it scans the system's fonts), so one
//! engine should be created off the UI thread and reused.
#![forbid(unsafe_code)]

use flare_document::{MAX_SEGMENTS, PathSegment};
use parley::style::{FontFamily, FontStyle, FontWeight, StyleProperty};
use parley::{Alignment, AlignmentOptions, FontContext, LayoutContext, PositionedLayoutItem};
use skrifa::instance::{LocationRef, Size};
use skrifa::outline::DrawSettings;
use skrifa::raw::types::F2Dot14;
use skrifa::{FontRef, GlyphId, MetadataProvider};
use thiserror::Error;

mod pen;
mod source;
#[cfg(test)]
mod tests;

use pen::Pen;
pub use source::Caret;

/// Most characters one text object may hold.
pub const MAX_CHARS: usize = flare_document::MAX_TEXT_CHARS;
/// Smallest and largest font size, in stage units.
pub const MIN_SIZE: f32 = flare_document::MIN_TEXT_SIZE;
pub const MAX_SIZE: f32 = flare_document::MAX_TEXT_SIZE;

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
    #[error("the text would leave the drawable range")]
    OutOfRange,
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

/// A font that is always there, so text can be set where no fonts are installed (a browser, a
/// bare container). It is the interface font, under the SIL Open Font License.
const BUNDLED_FONT: &[u8] = include_bytes!("../../../../assets/ui/fonts/SourceSans3-Regular.ttf");

/// The family name of the bundled font.
pub const BUNDLED_FAMILY: &str = "Source Sans 3";

/// The font list new text starts with: the system's sans-serif, else the bundled font.
pub const DEFAULT_FAMILY: &str = "sans-serif, Source Sans 3";

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
        let mut engine = Self {
            fonts: FontContext::new(),
            layouts: LayoutContext::new(),
        };
        engine.register_font(BUNDLED_FONT.to_vec());
        engine
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
        let (layout, path, glyphs) = self.lay_out(text, style)?;
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

    /// The layout of `text`, the outlines of its glyphs, and how many glyphs had one. Empty text
    /// lays out to nothing; judging that is the caller's.
    fn lay_out(
        &mut self,
        text: &str,
        style: &TextStyle,
    ) -> Result<(parley::Layout<()>, Vec<PathSegment>, usize), TextError> {
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
        Ok((layout, path, glyphs))
    }
}
