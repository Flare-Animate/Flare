//! The Selection tool: which span a click on the stage picks, and what Select All selects.

mod hit;
mod spans;

pub use hit::{locate_hit, picture_at};
pub use spans::spans_at_frame;

#[cfg(test)]
mod tests;
