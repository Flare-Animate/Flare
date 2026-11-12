#![allow(clippy::expect_used)]

use super::area::*;

/// A `size` square picture with the given pixels inked.
fn picture(size: usize, inked: impl Fn(usize, usize) -> bool) -> Vec<bool> {
    (0..size * size)
        .map(|i| inked(i % size, i / size))
        .collect()
}

/// A square ring of ink one pixel wide, its outer edge at `from` and `to` (inclusive).
fn ring(x: usize, y: usize, from: usize, to: usize) -> bool {
    let on_edge = x == from || x == to || y == from || y == to;
    on_edge && (from..=to).contains(&x) && (from..=to).contains(&y)
}

/// Whether the outlines wind around the middle of pixel `(x, y)`, counting crossings.
fn covers(outlines: &[Outline], x: usize, y: usize) -> bool {
    let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
    let mut inside = false;
    for outline in outlines {
        for i in 0..outline.len() {
            let a = outline[i];
            let b = outline[(i + 1) % outline.len()];
            if (a.1 > py) != (b.1 > py) && px < a.0 + (py - a.1) * (b.0 - a.0) / (b.1 - a.1) {
                inside = !inside;
            }
        }
    }
    inside
}

#[test]
fn a_closed_ring_encloses_its_inside_and_reaches_under_the_ink() {
    let ink = picture(40, |x, y| ring(x, y, 5, 30));
    let grid = InkGrid::new(40, 40, &ink).expect("grid");
    let area = enclosed_area(&grid, (15, 15), 0.0).expect("enclosed");
    assert!(covers(&area, 15, 15));
    assert!(
        covers(&area, 6, 6) && covers(&area, 29, 29),
        "corner to corner"
    );
    assert!(covers(&area, 5, 15), "a pixel under the line");
    assert!(
        !covers(&area, 3, 15) && !covers(&area, 35, 35),
        "not outside"
    );
}

#[test]
fn an_open_ring_has_no_inside_unless_its_gap_is_narrow_enough() {
    // The ring's left side is missing two pixels.
    let ink = picture(40, |x, y| {
        ring(x, y, 5, 30) && !(x == 5 && (17..19).contains(&y))
    });
    let grid = InkGrid::new(40, 40, &ink).expect("grid");
    assert_eq!(enclosed_area(&grid, (15, 15), 0.0), None, "it leaks out");
    let closed = enclosed_area(&grid, (15, 15), 4.0).expect("the gap is closed");
    assert!(covers(&closed, 15, 15) && covers(&closed, 8, 18));
    assert!(!covers(&closed, 2, 18), "nothing spills out of the gap");
}

#[test]
fn an_island_inside_the_area_is_left_as_a_hole() {
    let ink = picture(60, |x, y| ring(x, y, 5, 50) || ring(x, y, 20, 35));
    let grid = InkGrid::new(60, 60, &ink).expect("grid");
    let area = enclosed_area(&grid, (10, 10), 0.0).expect("enclosed");
    assert!(covers(&area, 10, 10) && covers(&area, 45, 45));
    assert!(
        !covers(&area, 27, 27),
        "the inside of the island is not this area"
    );
    assert!(area.len() >= 2, "an outer loop and a hole");
}

#[test]
fn a_click_on_ink_or_next_to_it_and_bad_input() {
    let ink = picture(40, |x, y| ring(x, y, 5, 30));
    let grid = InkGrid::new(40, 40, &ink).expect("grid");
    assert_eq!(enclosed_area(&grid, (5, 15), 0.0), None, "on the line");
    let near = enclosed_area(&grid, (6, 15), 6.0).expect("a click beside the line still fills");
    assert!(covers(&near, 15, 15));
    assert_eq!(enclosed_area(&grid, (40, 0), 0.0), None, "off the picture");
    assert!(
        enclosed_area(&grid, (15, 15), f64::NAN).is_some(),
        "a bad gap is none"
    );
    assert!(InkGrid::new(3, 3, &ink).is_none(), "the sizes must agree");
    assert!(InkGrid::new(0, 0, &[]).is_none());
}

#[test]
fn the_outline_is_straight_lines_not_a_staircase() {
    let ink = picture(40, |x, y| ring(x, y, 5, 30));
    let grid = InkGrid::new(40, 40, &ink).expect("grid");
    let area = enclosed_area(&grid, (15, 15), 0.0).expect("enclosed");
    let points: usize = area.iter().map(Vec::len).sum();
    assert!(points <= 8, "a square is a few corners, got {points}");
}
