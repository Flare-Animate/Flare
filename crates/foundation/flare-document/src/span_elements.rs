//! A frame span holding several elements: what Animate keeps in one keyframe of one layer.

use crate::{Content, FrameSpan, TimelineError, Tween};

/// The most elements one frame span may hold, the first included.
pub const MAX_ELEMENTS: usize = 10_000;

impl FrameSpan {
    /// The same span with `extras` drawn above its content, in order. Only a shape or an instance
    /// can be one of several elements, so neither the content nor an extra may be empty or a
    /// camera. A tween is the first element's: the others are held.
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
        self.extras = extras;
        Ok(self)
    }

    /// The same span holding exactly `elements`, bottom first, as its content and extras. `None`
    /// for no elements; an error when one is not allowed (see [`Self::with_extras`]). A single
    /// element keeps the span's motion; a span that gets several keeps it too while its first
    /// element is still the same kind of thing (a tween is the first element's), else it is held.
    pub fn with_elements(&self, elements: Vec<Content>) -> Result<Option<Self>, TimelineError> {
        let mut elements = elements.into_iter();
        let Some(first) = elements.next() else {
            return Ok(None);
        };
        let extras: Vec<Content> = elements.collect();
        let mut span = self.clone();
        let same_kind = std::mem::discriminant(&span.content) == std::mem::discriminant(&first);
        span.content = first;
        span.extras = Vec::new();
        if !extras.is_empty() {
            if !same_kind {
                span.tween = Tween::None;
                span.ease = 0;
            }
            span = span.with_extras(extras)?;
        }
        Ok(Some(span))
    }

    /// The same span with the element at `index` replaced. `None` when there is no such element
    /// or the replacement is not allowed there (a span of several holds only shapes and
    /// instances).
    #[must_use]
    pub fn with_element(&self, index: usize, content: Content) -> Option<Self> {
        let mut all: Vec<Content> = self.elements().cloned().collect();
        *all.get_mut(index)? = content;
        self.with_elements(all).ok().flatten()
    }

    /// The same span without the element at `index`; `None` when there is no such element or
    /// it is the only one (remove the span instead).
    #[must_use]
    pub fn without_element(&self, index: usize) -> Option<Self> {
        if self.element_count() < 2 || index >= self.element_count() {
            return None;
        }
        let mut all: Vec<Content> = self.elements().cloned().collect();
        all.remove(index);
        self.with_elements(all).ok().flatten()
    }

    /// The same span with `content` drawn above every element it has. An empty keyframe takes
    /// it as its content. `None` when the span cannot hold it (a tween, a camera, too many).
    #[must_use]
    pub fn with_appended(&self, content: Content) -> Option<Self> {
        if matches!(self.content, Content::Empty) && self.extras.is_empty() {
            return self.with_elements(vec![content]).ok().flatten();
        }
        // Appending is refused, not silently un-tweened.
        if self.tween != Tween::None {
            return None;
        }
        let mut all: Vec<Content> = self.elements().cloned().collect();
        all.push(content);
        self.with_elements(all).ok().flatten()
    }

    /// The same span with the element at `from` moved to `to` in the stacking order.
    #[must_use]
    pub fn with_element_moved(&self, from: usize, to: usize) -> Option<Self> {
        let mut all: Vec<Content> = self.elements().cloned().collect();
        if from >= all.len() || to >= all.len() {
            return None;
        }
        let item = all.remove(from);
        all.insert(to, item);
        self.with_elements(all).ok().flatten()
    }

    /// The same span, every element and its motion kept, over other frames.
    #[must_use]
    pub fn retimed(&self, start: u32, duration: std::num::NonZeroU32) -> Self {
        let mut span = self.clone();
        span.start = start;
        span.duration = duration;
        span
    }

    /// The same span held: no tween and no easing, as a span cut short loses what it led into.
    #[must_use]
    pub fn held(&self) -> Self {
        let mut span = self.clone();
        span.tween = Tween::None;
        span.ease = 0;
        span
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
