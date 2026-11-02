//! The Pen tool: the path under construction, and the shape a finished path makes.

mod path;
mod shape;

pub use path::*;
pub use shape::{PenStyle, path_shape};

#[cfg(test)]
mod tests;
