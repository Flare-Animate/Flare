//! What a text object is made from.
//!
//! Text is a [`Shape`](crate::Shape) whose path is the outline of the letters, so drawing,
//! hit testing and every export need no idea that text exists. The shape also carries the
//! [`TextSource`], the words and the font they were set in, so the text can be edited again: the
//! outline is regenerated from the source and is only ever a copy of it.

use flare_core::DocumentPoint;
use thiserror::Error;

/// Most characters one text object may hold.
pub const MAX_TEXT_CHARS: usize = 10_000;

/// Smallest and largest font size, in stage units.
pub const MIN_TEXT_SIZE: f32 = 1.0;
pub const MAX_TEXT_SIZE: f32 = 2_048.0;

/// Longest font family list, in characters.
pub const MAX_FAMILY_CHARS: usize = 256;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TextSourceError {
    #[error("there is no text")]
    Empty,
    #[error("text is limited to {MAX_TEXT_CHARS} characters")]
    TooLong,
    #[error("the font size must be between {MIN_TEXT_SIZE} and {MAX_TEXT_SIZE}")]
    BadSize,
    #[error("a font family list is limited to {MAX_FAMILY_CHARS} characters")]
    FamilyTooLong,
}

/// The words of a text object and how they are set.
#[derive(Debug, Clone, PartialEq)]
pub struct TextSource {
    text: String,
    /// A CSS-style family list, such as `"Helvetica, Arial, sans-serif"`.
    family: String,
    size: f32,
    bold: bool,
    italic: bool,
    /// Where the top-left corner of the first line is.
    origin: DocumentPoint,
}

impl TextSource {
    pub fn new(
        text: impl Into<String>,
        family: impl Into<String>,
        size: f32,
        bold: bool,
        italic: bool,
        origin: DocumentPoint,
    ) -> Result<Self, TextSourceError> {
        let (text, family) = (text.into(), family.into());
        if text.trim().is_empty() {
            return Err(TextSourceError::Empty);
        }
        if text.chars().count() > MAX_TEXT_CHARS {
            return Err(TextSourceError::TooLong);
        }
        if !size.is_finite() || !(MIN_TEXT_SIZE..=MAX_TEXT_SIZE).contains(&size) {
            return Err(TextSourceError::BadSize);
        }
        if family.chars().count() > MAX_FAMILY_CHARS {
            return Err(TextSourceError::FamilyTooLong);
        }
        Ok(Self {
            text,
            family,
            size,
            bold,
            italic,
            origin,
        })
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn family(&self) -> &str {
        &self.family
    }

    pub fn size(&self) -> f32 {
        self.size
    }

    pub fn bold(&self) -> bool {
        self.bold
    }

    pub fn italic(&self) -> bool {
        self.italic
    }

    pub fn origin(&self) -> DocumentPoint {
        self.origin
    }

    /// The same text moved to `origin`.
    #[must_use]
    pub fn at(mut self, origin: DocumentPoint) -> Self {
        self.origin = origin;
        self
    }

    /// The source after its outline was moved to `origin` and scaled by `scale` (the same on both
    /// axes), or `None` when that is not a size text can have.
    pub fn moved_and_scaled(&self, origin: DocumentPoint, scale: f64) -> Option<Self> {
        let size = (f64::from(self.size) * scale) as f32;
        Self::new(
            self.text.clone(),
            self.family.clone(),
            size,
            self.bold,
            self.italic,
            origin,
        )
        .ok()
    }
}

#[cfg(test)]
#[path = "text_tests.rs"]
mod tests;
