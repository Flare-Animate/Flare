//! What several tools share: finding a span, where things are, copies of what a span holds, and
//! path editing. A tool's own folder holds everything else about it.

pub mod bounds;
pub(crate) mod content;
mod many;
pub(crate) mod path_edit;
mod span;

pub(crate) use bounds::content_bounds;
pub use bounds::{shape_bounds, span_bounds, spans_bounds, spans_bounds_at, symbol_bounds};
pub(crate) use content::{fresh_content, new_layer_with, transform_shape};
pub(crate) use many::by_span;
pub use many::{each_element, each_element_with};
pub(crate) use span::scene_timeline;
pub use span::{SpanRef, find_element, find_span};
pub(crate) use span::{remove_element, replace_element, replace_span};

#[cfg(test)]
mod tests;
