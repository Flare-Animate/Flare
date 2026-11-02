use super::*;

#[test]
fn only_the_frames_in_view_are_visited() {
    // Frame 0 starts at x = 100 and each frame is 10 wide; the view is x 250..=330.
    assert_eq!(visible_frames(250.0, 330.0, 100.0, 10.0, 1000), 15..23);
}

#[test]
fn a_view_left_of_the_track_starts_at_the_first_frame() {
    assert_eq!(visible_frames(0.0, 130.0, 100.0, 10.0, 1000), 0..3);
}

#[test]
fn frames_past_the_end_are_not_visited() {
    assert_eq!(visible_frames(0.0, 10_000.0, 0.0, 10.0, 24), 0..24);
    assert_eq!(visible_frames(5_000.0, 6_000.0, 0.0, 10.0, 24), 24..24);
}

#[test]
fn nonsense_widths_visit_nothing() {
    for width in [0.0, -3.0, f32::NAN, f32::INFINITY] {
        assert_eq!(visible_frames(0.0, 100.0, 0.0, width, 24), 0..0, "{width}");
    }
    assert_eq!(visible_frames(f32::NAN, 100.0, 0.0, 10.0, 24), 0..0);
}
