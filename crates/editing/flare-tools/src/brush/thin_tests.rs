#![allow(clippy::expect_used)]

use super::*;

/// The two sides of a stroke along `centre` points, `half` wide each way, across the y axis.
fn sides(centre: &[(f64, f64)], half: f64) -> (Vec<DVec2>, Vec<DVec2>) {
    let left = centre
        .iter()
        .map(|&(x, y)| DVec2::new(x, y + half))
        .collect();
    let right = centre
        .iter()
        .map(|&(x, y)| DVec2::new(x, y - half))
        .collect();
    (left, right)
}

#[test]
fn a_straight_run_keeps_only_its_ends() {
    let centre: Vec<(f64, f64)> = (0..=40).map(|i| (f64::from(i) * 0.25, 0.0)).collect();
    let (left, right) = sides(&centre, 2.0);
    assert_eq!(thin(&left, &right), vec![0, 40]);
}

#[test]
fn a_curve_keeps_every_point_left_out_within_the_sag_of_what_is_kept() {
    // A circle of radius 50 cut a quarter unit at a time: about 1260 points.
    let centre: Vec<(f64, f64)> = (0..=1256)
        .map(|i| {
            let a = f64::from(i) * 0.005;
            (50.0 * a.cos(), 50.0 * a.sin())
        })
        .collect();
    let (left, right) = sides(&centre, 1.5);
    let kept = thin(&left, &right);
    assert!(
        kept.len() * 4 < centre.len(),
        "{} of {} kept",
        kept.len(),
        centre.len()
    );
    for pair in kept.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        for i in a + 1..b {
            assert!(off(left[i], left[a], left[b]) <= OUTLINE_SAG + 1e-12);
            assert!(off(right[i], right[a], right[b]) <= OUTLINE_SAG + 1e-12);
        }
    }
}

#[test]
fn a_hairline_narrower_than_the_sag_keeps_its_shape() {
    // A wavy line a hundredth of a unit wide: a fixed sag would flatten every wave.
    let centre: Vec<(f64, f64)> = (0..=200)
        .map(|i| (f64::from(i) * 0.1, (f64::from(i) * 0.3).sin() * 0.05))
        .collect();
    let (left, right) = sides(&centre, 0.005);
    let kept = thin(&left, &right);
    for pair in kept.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        for i in a + 1..b {
            assert!(
                off(left[i], left[a], left[b]) <= 0.001 + 1e-12,
                "within a tenth of 0.01"
            );
        }
    }
    assert!(kept.len() > 20, "the waves are kept");
}

#[test]
fn short_or_unequal_sides_are_kept_whole() {
    let (left, right) = sides(&[(0.0, 0.0), (1.0, 0.0)], 1.0);
    assert_eq!(thin(&left, &right), vec![0, 1]);
    let (left, _) = sides(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)], 1.0);
    assert_eq!(thin(&left, &left[..2]), vec![0, 1, 2]);
}
