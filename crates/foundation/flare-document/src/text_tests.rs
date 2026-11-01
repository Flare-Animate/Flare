#![allow(clippy::expect_used)]

use super::*;
use crate::{PathSegment, Shape};
use flare_core::{Rgba8, ShapeId};

fn origin(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("point")
}

fn source() -> TextSource {
    TextSource::new("Hello", "Helvetica", 24.0, false, true, origin(10.0, 20.0)).expect("source")
}

#[test]
fn a_source_needs_words_a_size_in_range_and_bounded_lengths() {
    let make = |text: &str, family: &str, size: f32| {
        TextSource::new(text, family, size, false, false, origin(0.0, 0.0))
    };
    assert_eq!(make("", "A", 12.0), Err(TextSourceError::Empty));
    assert_eq!(make("  \n ", "A", 12.0), Err(TextSourceError::Empty));
    assert_eq!(make("x", "A", 0.5), Err(TextSourceError::BadSize));
    assert_eq!(make("x", "A", f32::NAN), Err(TextSourceError::BadSize));
    assert_eq!(
        make("x", "A", MAX_TEXT_SIZE + 1.0),
        Err(TextSourceError::BadSize)
    );
    assert_eq!(
        make(&"x".repeat(MAX_TEXT_CHARS + 1), "A", 12.0),
        Err(TextSourceError::TooLong)
    );
    assert_eq!(
        make("x", &"f".repeat(MAX_FAMILY_CHARS + 1), 12.0),
        Err(TextSourceError::FamilyTooLong)
    );
    assert!(make(&"x".repeat(MAX_TEXT_CHARS), "A", 12.0).is_ok());
}

#[test]
fn a_shape_remembers_its_text_until_it_is_taken_apart() {
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)
        .expect("rect")
        .with_text(source());
    assert_eq!(shape.text().map(TextSource::text), Some("Hello"));
    assert!(shape.clone().text_like(&shape).text().is_some());
    let apart = shape.clone().without_text();
    assert!(apart.text().is_none());
    assert_eq!(apart.path(), shape.path(), "the letters stay");
    // A rebuilt path is a new shape: it does not claim to be the text.
    let rebuilt = Shape::new(
        shape.id(),
        vec![PathSegment::MoveTo(origin(0.0, 0.0))],
        Rgba8::WHITE,
    )
    .expect("shape")
    .painted_like(&shape);
    assert!(rebuilt.text().is_none());
}

#[test]
fn moving_and_scaling_keeps_the_text_and_scales_its_size() {
    let moved = source()
        .moved_and_scaled(origin(100.0, 50.0), 2.0)
        .expect("moved");
    assert_eq!(moved.size(), 48.0);
    assert_eq!(moved.origin(), origin(100.0, 50.0));
    assert_eq!(moved.text(), "Hello");
    assert!(moved.italic() && !moved.bold());
    // Scaled out of range, it can no longer be text.
    assert!(
        source()
            .moved_and_scaled(origin(0.0, 0.0), 1000.0)
            .is_none()
    );
    assert!(source().moved_and_scaled(origin(0.0, 0.0), 0.0).is_none());
}
