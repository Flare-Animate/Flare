use super::*;
use crate::experimental::viewspace::neighbours::Neighbour;

fn nb(d: f32) -> Neighbour {
    Neighbour {
        index: 0,
        distance: d,
        weight: 1.0,
    }
}

#[test]
fn angular_confidence_falls_smoothly() {
    assert_eq!(angular_confidence(0.0), 1.0);
    assert!((angular_confidence(REFERENCE) - 0.5).abs() < 1e-6);
    assert!(angular_confidence(1.0) < angular_confidence(0.5));
    assert_eq!(angular_confidence(-1.0), 1.0);
}

#[test]
fn vertex_confidence_combines_distance_and_coverage() {
    assert_eq!(vertex_confidence(&[nb(0.0)], 1.0), 1.0);
    assert_eq!(
        vertex_confidence(&[nb(0.0)], 0.0),
        0.0,
        "never seen means no confidence"
    );
    assert!(vertex_confidence(&[nb(1.0)], 1.0) < 0.5);
    assert_eq!(vertex_confidence(&[], 1.0), 0.0);
    assert_eq!(
        vertex_confidence(&[nb(0.0)], 7.0),
        1.0,
        "coverage is clamped"
    );
    assert_eq!(
        vertex_confidence(&[nb(0.5), nb(0.1)], 1.0),
        angular_confidence(0.1),
        "nearest counts"
    );
}

#[test]
fn heat_colour_ramp() {
    assert_eq!(heat_colour(0.0)[..2], [1.0, 0.0]);
    assert_eq!(heat_colour(1.0)[..2], [0.0, 1.0]);
    assert_eq!(heat_colour(5.0), heat_colour(1.0));
    assert_eq!(heat_colour(f32::NAN).len(), 4);
    assert_eq!(mean(&[]), 0.0);
    assert_eq!(mean(&[0.0, 1.0]), 0.5);
}

#[test]
fn next_view_goes_where_coverage_is_missing() {
    let have = [
        ViewParams::from_degrees(0.0, None),
        ViewParams::from_degrees(90.0, None),
    ];
    let (v, gap) = suggest_next_view(&have, &[0.0], 15.0).unwrap_or_default_view();
    assert!(
        (v.azimuth.to_degrees() + 135.0).abs() < 16.0
            || (v.azimuth.to_degrees() - 180.0).abs() < 1.0,
        "{v:?}"
    );
    assert!(gap > 0.9);
}

#[test]
fn next_view_with_nothing_or_nonsense() {
    let (_, gap) = suggest_next_view(&[], &[0.0], 30.0).unwrap_or_default_view();
    assert!((gap - std::f32::consts::PI).abs() < 1e-6);
    assert!(suggest_next_view(&[], &[0.0], 0.0).is_none());
    assert!(suggest_next_view(&[], &[0.0], f32::NAN).is_none());
    assert!(suggest_next_view(&[], &[], 30.0).is_none());
    assert!(suggest_next_view(&[], &[f32::NAN, 120.0], 30.0).is_none());
}

trait OrDefault {
    fn unwrap_or_default_view(self) -> (ViewParams, f32);
}
impl OrDefault for Option<(ViewParams, f32)> {
    fn unwrap_or_default_view(self) -> (ViewParams, f32) {
        self.unwrap_or((ViewParams::default(), -1.0))
    }
}
