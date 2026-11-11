//! The Free Transform tool: the handles around a selection, and moving a span through a matrix.

mod apply;
mod grab;
pub mod handles;

pub use apply::{transform, transform_many, translate, translate_many};
pub use grab::{Grab, GrabAxis, grab_at};

#[cfg(test)]
mod grab_tests;
#[cfg(test)]
mod tests;
