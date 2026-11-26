//! Merge Drawing on hostile input: a crowd, and coordinates at the edge of what a shape may say.
#![allow(clippy::expect_used)]

use super::*;

fn square(x: f64, y: f64, size: f64, fill: Rgba8) -> Shape {
    Shape::rect(ShapeId::new(), x, y, size, size, fill)
        .expect("rect")
        .merged(true)
}

#[test]
fn a_batch_past_the_keyframe_limit_is_refused_without_cutting_it_all() {
    // Apart from each other, so nothing cuts anything and every shape stays a piece.
    let crowd: Vec<Shape> = (0..=MAX_ELEMENTS)
        .map(|i| square(i as f64 * 10.0, 0.0, 5.0, Rgba8::new(i as u8, 1, 2, 255)))
        .collect();
    assert_eq!(merge_into(Vec::new(), crowd), Err(TooManyPieces));
}

#[test]
fn a_keyframe_already_full_refuses_one_more_shape() {
    let held: Vec<Content> = (0..MAX_ELEMENTS)
        .map(|i| Content::Shape(square(i as f64 * 10.0, 0.0, 5.0, Rgba8::WHITE)))
        .collect();
    assert!(merge_into(held.clone(), vec![square(-50.0, 0.0, 5.0, Rgba8::WHITE)]).is_err());
    assert!(merge_into(held, Vec::new()).is_ok(), "what is there stays");
}

#[test]
fn coordinates_at_the_edge_neither_panic_nor_produce_an_invalid_shape() {
    for size in [1.0e12, 1.0e150, 1.0e-12] {
        let first = merge_into(
            Vec::new(),
            vec![square(-size, -size, size * 2.0, Rgba8::WHITE)],
        )
        .expect("first");
        let both = merge_into(
            first.elements,
            vec![square(0.0, 0.0, size, Rgba8::opaque(1, 2, 3))],
        );
        if let Ok(both) = both {
            for element in &both.elements {
                assert!(
                    matches!(element, Content::Shape(s) if s.path().len() <= flare_document::MAX_SEGMENTS)
                );
            }
        }
    }
}
