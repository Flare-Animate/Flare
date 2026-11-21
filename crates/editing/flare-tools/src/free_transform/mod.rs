//! The Free Transform tool: the handles around a selection, and moving a span through a matrix.

mod apply;
mod at_frame;
mod grab;
pub mod handles;
mod place;

pub use apply::{transform, transform_many, translate, translate_many};
pub use at_frame::{own_held, transform_many_at, translate_many_at};
pub use grab::{Grab, GrabAxis, grab_at};
pub use place::{PlacementError, placement};

#[cfg(test)]
mod at_frame_tests;
#[cfg(test)]
mod grab_tests;
#[cfg(test)]
mod place_tests;
#[cfg(test)]
mod tests;
