//! The Free Transform tool: the handles around a selection, and moving a span through a matrix.

mod apply;
pub mod handles;

pub use apply::{transform, transform_many, translate, translate_many};

#[cfg(test)]
mod tests;
