//! A frame span holding several elements: what Animate keeps in one keyframe of one layer.

use crate::{Content, FrameSpan, TimelineError, Tween};

/// The most elements one frame span may hold, the first included.
pub const MAX_ELEMENTS: usize = 10_000;

impl FrameSpan {
    /// The same span with `extras` drawn above its content, in order. Only a shape or an instance
    /// can be one of several elements, so neither the content nor an extra may be empty or a
    /// camera, and a span that tweens holds one element.
    pub fn with_extras(mut self, extras: Vec<Content>) -> Result<Self, TimelineError> {
        if extras.is_empty() {
            self.extras = extras;
            return Ok(self);
        }
        let count = extras.len().saturating_add(1);
        if count > MAX_ELEMENTS {
            return Err(TimelineError::TooManyElements(count));
        }
        let element = |c: &Content| matches!(c, Content::Shape(_) | Content::Instance(_));
        if !element(self.content()) || !extras.iter().all(element) {
            return Err(TimelineError::NotAnElement);
        }
        if self.tween != Tween::None {
            return Err(TimelineError::ManyElementsTween);
        }
        self.extras = extras;
        Ok(self)
    }

    /// The elements drawn above the first, in order from the bottom.
    pub fn extras(&self) -> &[Content] {
        &self.extras
    }

    /// How many elements the span holds.
    pub fn element_count(&self) -> usize {
        1 + self.extras.len()
    }

    /// Every element, the first and then the extras, from the bottom up.
    pub fn elements(&self) -> impl Iterator<Item = &Content> {
        std::iter::once(self.content()).chain(self.extras.iter())
    }

    /// The element at `index` of `elements()`.
    pub fn element(&self, index: usize) -> Option<&Content> {
        match index {
            0 => Some(self.content()),
            n => self.extras.get(n - 1),
        }
    }
}

#[cfg(test)]
#[path = "span_elements_tests.rs"]
mod tests;
