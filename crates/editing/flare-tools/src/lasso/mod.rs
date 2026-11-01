//! The Lasso and Polygon Lasso tools: a loop drawn freehand or corner by corner, and the spans it
//! encloses. The two share what a loop selects, so they live together.

mod contain;
pub mod freehand;
pub mod polygon;

pub use contain::{polygon_contains, spans_in_polygon};

#[cfg(test)]
mod tests;
