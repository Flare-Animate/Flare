use super::*;

/// 20 layers of 50 frames at 10 points a frame: 640 by 420 points of content.
fn content() -> Rect {
    Rect::from_min_size(
        Pos2::new(100.0, 50.0),
        Vec2::new(LABEL_W + 500.0, ROW * 21.0),
    )
}

#[test]
fn unscrolled_the_names_and_ruler_sit_at_the_contents_edges() {
    let l = Layout::new(content(), content(), 10.0);
    assert_eq!(l.labels_left, 100.0);
    assert_eq!(l.label_edge, 240.0);
    assert_eq!(l.track_left, 240.0);
    assert_eq!(l.ruler_top, 50.0);
    assert_eq!(l.frame_at(265.0), 2.5);
    assert_eq!(l.frame_x(3.0), 270.0);
    assert_eq!(l.row_at(Pos2::new(300.0, 55.0)), None, "the ruler");
    assert_eq!(l.row_at(Pos2::new(300.0, 75.0)), Some(0));
    assert!(l.on_labels(Pos2::new(239.0, 75.0)));
    assert!(!l.on_labels(Pos2::new(241.0, 75.0)));
}

#[test]
fn scrolled_the_names_and_ruler_stay_in_view_and_cover_what_is_under_them() {
    // The view shows the content from 300 points right and 100 points down.
    let view = Rect::from_min_size(Pos2::new(400.0, 150.0), Vec2::new(300.0, 200.0));
    let l = Layout::new(content(), view, 10.0);
    assert_eq!(l.labels_left, 400.0, "names at the left of the view");
    assert_eq!(l.label_edge, 540.0);
    assert_eq!(l.ruler_top, 150.0, "the ruler at its top");
    assert_eq!(
        l.track_left, 240.0,
        "frames still count from the content's start"
    );
    assert_eq!(l.frame_at(545.0), 30.5);
    // A point over the pinned names is a name, though frames scrolled under it.
    assert!(l.on_labels(Pos2::new(500.0, 200.0)));
    // A point on the pinned ruler is the ruler, though a row scrolled under it.
    assert_eq!(l.row_at(Pos2::new(600.0, 160.0)), None);
    assert_eq!(
        l.row_at(Pos2::new(600.0, 175.0)),
        Some(5),
        "the first row below it"
    );
    assert_eq!(l.row_top(5), 170.0);
}

#[test]
fn the_onion_markers_bracket_the_ghosts_and_a_press_near_one_takes_it() {
    let l = Layout::new(content(), content(), 10.0);
    // Playhead on frame 5, two frames each side: ghosts 3 and 4, 6 and 7.
    let (start, end) = l.onion_markers(5, 2, 2);
    assert_eq!((start, end), (l.frame_x(3.0), l.frame_x(8.0)));
    let on_ruler = |x: f32| Pos2::new(x, l.ruler_top + 15.0);
    assert_eq!(
        l.onion_marker_at(on_ruler(start + 3.0), 5, 2, 2),
        Some(OnionEnd::Before)
    );
    assert_eq!(
        l.onion_marker_at(on_ruler(end - 4.0), 5, 2, 2),
        Some(OnionEnd::After)
    );
    assert_eq!(
        l.onion_marker_at(on_ruler(start + 20.0), 5, 2, 2),
        None,
        "between them"
    );
    let below = Pos2::new(start, l.ruler_top + ROW + 5.0);
    assert_eq!(l.onion_marker_at(below, 5, 2, 2), None, "not on the ruler");
    // Near the first frame the range is cut at frame 0.
    assert_eq!(l.onion_markers(1, 4, 0).0, l.frame_x(0.0));
}

#[test]
fn dragging_a_marker_sets_its_side_of_the_range_within_the_limit() {
    let l = Layout::new(content(), content(), 10.0);
    // Playhead on frame 10, one a side.
    let before = l.onion_dragged(OnionEnd::Before, l.frame_x(6.0), 10, (1, 1), 12);
    assert_eq!(before, (4, 1), "the start dragged to frame 6");
    let after = l.onion_dragged(OnionEnd::After, l.frame_x(14.0), 10, (1, 1), 12);
    assert_eq!(after, (1, 3), "the end dragged to the edge after frame 13");
    let far = l.onion_dragged(OnionEnd::After, l.frame_x(90.0), 10, (1, 1), 12);
    assert_eq!(far, (1, 12), "no more than the limit");
    let past = l.onion_dragged(OnionEnd::Before, l.frame_x(30.0), 10, (1, 1), 12);
    assert_eq!(past, (0, 1), "a start dragged past the playhead is none");
}
