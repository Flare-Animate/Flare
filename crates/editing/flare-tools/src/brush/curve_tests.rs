#![allow(clippy::expect_used)]

use glamx::DVec2;

use super::curve::{MIN_STEPS, cap, cap_steps, smooth};
use super::*;

fn spine(points: &[(f64, f64, f64)]) -> Vec<Spine> {
    points
        .iter()
        .map(|&(x, y, half)| Spine {
            at: DVec2::new(x, y),
            half,
        })
        .collect()
}

/// The biggest change in slope of the width between neighbouring pieces: a corner in the edge
/// shows as a jump.
fn worst_width_kink(line: &[Spine]) -> f64 {
    let slopes: Vec<f64> = line
        .windows(2)
        .filter_map(|w| {
            let run = (w[1].at - w[0].at).length();
            (run > 1e-9).then(|| (w[1].half - w[0].half) / run)
        })
        .collect();
    slopes
        .windows(2)
        .map(|s| (s[1] - s[0]).abs())
        .fold(0.0, f64::max)
}

#[test]
fn the_width_bends_smoothly_through_a_kept_point() {
    // Thin, then thick, then thin, the points far apart: a straight ramp between them turned
    // the edge by a whole ramp's slope at the middle point.
    let line = smooth(&spine(&[
        (0.0, 0.0, 1.0),
        (50.0, 0.0, 6.0),
        (100.0, 0.0, 1.0),
    ]));
    let ramp = 5.0 / 50.0;
    assert!(
        worst_width_kink(&line) < ramp / 4.0,
        "a corner of {} against a ramp of {ramp}",
        worst_width_kink(&line)
    );
}

#[test]
fn the_width_never_bulges_past_the_widths_it_joins() {
    let line = smooth(&spine(&[
        (0.0, 0.0, 1.0),
        (2.0, 0.0, 8.0),
        (40.0, 0.0, 8.0),
        (41.0, 0.0, 0.5),
        (90.0, 0.0, 3.0),
    ]));
    assert!(line.iter().all(|s| (0.5..=8.0).contains(&s.half)));
    // A held width stays held: no wave between two equal widths.
    let held: Vec<f64> = line
        .iter()
        .filter(|s| (3.0..=39.0).contains(&s.at.x))
        .map(|s| s.half)
        .collect();
    assert!(held.iter().all(|h| (h - 8.0).abs() < 1e-9), "{held:?}");
}

#[test]
fn a_wide_sweep_through_few_points_turns_in_small_steps() {
    // Eight points round a circle of radius 200: four pieces a span turned about eleven degrees
    // at every corner, which shows as flats when zoomed in.
    let points: Vec<(f64, f64, f64)> = (0..=8)
        .map(|k| {
            let a = std::f64::consts::TAU * f64::from(k) / 8.0;
            (200.0 * a.cos(), 200.0 * a.sin(), 5.0)
        })
        .collect();
    let line = smooth(&spine(&points));
    let worst = line
        .windows(3)
        .map(|w| (w[1].at - w[0].at).angle_to(w[2].at - w[1].at).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.05, "a corner of {} degrees", worst.to_degrees());
}

#[test]
fn a_short_straight_run_costs_no_more_than_before() {
    let line = smooth(&spine(&[(0.0, 0.0, 2.0), (1.0, 0.0, 2.0), (2.0, 0.0, 2.0)]));
    assert_eq!(line.len(), 2 * MIN_STEPS + 1);
}

#[test]
fn a_round_end_has_as_many_pieces_as_its_size_needs() {
    assert_eq!(cap_steps(0.5), 8);
    assert!(cap_steps(100.0) > 40);
    assert_eq!(cap_steps(1e12), 96);
    assert_eq!(cap_steps(f64::NAN), 8);
    let radius = 50.0;
    let end = cap(DVec2::ZERO, DVec2::Y, radius, Nib::ROUND);
    let mut ring = vec![DVec2::Y * radius];
    ring.extend(end);
    ring.push(-DVec2::Y * radius);
    for piece in ring.windows(2) {
        let middle = (piece[0] + piece[1]) / 2.0;
        assert!(radius - middle.length() <= 0.02 + 1e-9);
    }
}

#[test]
fn the_ends_of_an_arc_keep_curving() {
    // A quarter circle through four points: carried on straight, the first and last spans
    // flattened and fell a unit and more inside the circle.
    let points: Vec<(f64, f64, f64)> = (0..=3)
        .map(|k| {
            let a = std::f64::consts::FRAC_PI_2 * f64::from(k) / 3.0;
            (100.0 * a.cos(), 100.0 * a.sin(), 2.0)
        })
        .collect();
    let line = smooth(&spine(&points));
    let worst = line
        .iter()
        .map(|s| (100.0 - s.at.length()).abs())
        .fold(0.0, f64::max);
    assert!(worst < 0.3, "strays {worst} from the arc");
    // Two points have no turn to carry on: still a straight line.
    let straight = smooth(&spine(&[(0.0, 0.0, 1.0), (10.0, 0.0, 1.0)]));
    assert!(straight.iter().all(|s| s.at.y.abs() < 1e-9));
}
