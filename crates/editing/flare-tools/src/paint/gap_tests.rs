#![allow(clippy::expect_used)]

use super::area::{InkGrid, enclosed_area};
use super::gap::*;

#[test]
fn each_size_closes_a_wider_gap_than_the_one_before_and_none_closes_nothing() {
    assert_eq!(GapSize::DontClose.points(), 0.0);
    for pair in GapSize::ALL.windows(2) {
        assert!(pair[0].points() < pair[1].points(), "{pair:?}");
    }
    assert_eq!(GapSize::default(), GapSize::Small, "Animate's default");
}

#[test]
fn a_gap_is_measured_on_the_screen_so_zooming_in_closes_less_of_the_stage() {
    let small = GapSize::Small;
    assert_eq!(small.on_stage(1.0), small.points());
    assert_eq!(small.on_stage(2.0), small.points() / 2.0);
    assert_eq!(small.on_stage(0.5), small.points() * 2.0);
    // Zoomed far out, it stops at the most a gap may be on the stage.
    assert_eq!(GapSize::Large.on_stage(0.01), MAX_GAP);
    // A scale that is no scale counts as one.
    for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(small.on_stage(bad), small.points(), "{bad}");
    }
    assert_eq!(GapSize::DontClose.on_stage(0.01), 0.0);
}

#[test]
fn a_wider_size_fills_a_ring_a_narrower_one_leaves_open() {
    // A square ring with 16 pixels missing from its left side.
    let size = 80;
    let ink: Vec<bool> = (0..size * size)
        .map(|i| {
            let (x, y) = (i % size, i / size);
            let edge = x == 5 || x == 70 || y == 5 || y == 70;
            let on = edge && (5..=70).contains(&x) && (5..=70).contains(&y);
            on && !(x == 5 && (30..46).contains(&y))
        })
        .collect();
    let grid = InkGrid::new(size, size, &ink).expect("grid");
    let fills = |gap: GapSize| enclosed_area(&grid, (40, 40), gap.on_stage(1.0)).is_some();
    assert!(!fills(GapSize::DontClose));
    assert!(!fills(GapSize::Small), "10 points do not reach across 16");
    assert!(fills(GapSize::Medium));
    assert!(fills(GapSize::Large));
}
