//! The Selection, Lasso and Polygon Lasso tools: which spans a click, a loop or a frame picks.

mod hit;
mod spans;

pub use hit::locate_hit;
pub use spans::{polygon_contains, spans_at_frame, spans_in_polygon};

#[cfg(test)]
mod tests;
