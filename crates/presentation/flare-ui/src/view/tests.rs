use super::*;

fn panel() -> Rect {
    Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(800.0, 400.0))
}

const EXTENT: Vec2 = Vec2::new(400.0, 200.0);

fn near(a: Pos2, b: Pos2) -> bool {
    (a - b).length() < 0.01
}

#[test]
fn default_view_fits_the_stage_centered_in_the_panel() {
    let view = StageView::default();
    let rect = view.image_rect(panel(), EXTENT);
    assert_eq!(view.scale(panel(), EXTENT), 2.0);
    assert_eq!(rect.size(), Vec2::new(800.0, 400.0));
    assert_eq!(rect.center(), panel().center());
    assert!(view.is_fit());
}

#[test]
fn zooming_keeps_the_point_under_the_cursor_fixed() {
    let mut view = StageView::default();
    let anchor = Pos2::new(250.0, 120.0);
    let at = |view: &StageView| {
        let rect = view.image_rect(panel(), EXTENT);
        (anchor - rect.min) / view.scale(panel(), EXTENT)
    };
    let before = at(&view);
    view.zoom_about(anchor, 3.0, panel(), EXTENT);
    let after = at(&view);
    assert!(
        near(Pos2::ZERO + before, Pos2::ZERO + after),
        "{before:?} {after:?}"
    );
    assert_eq!(view.zoom(), 3.0);
    view.zoom_about(anchor, 1.0 / 3.0, panel(), EXTENT);
    assert!(view.zoom() > 0.99 && view.zoom() < 1.01);
    assert!(near(Pos2::ZERO + at(&view), Pos2::ZERO + before));
}

#[test]
fn zoom_is_clamped_and_bad_factors_are_ignored() {
    let mut view = StageView::default();
    view.zoom_center(1000.0, panel(), EXTENT);
    assert_eq!(view.zoom(), MAX_ZOOM);
    view.zoom_center(0.0001, panel(), EXTENT);
    assert_eq!(view.zoom(), MIN_ZOOM);
    let before = view;
    for bad in [f32::NAN, f32::INFINITY, 0.0, -2.0] {
        view.zoom_center(bad, panel(), EXTENT);
    }
    assert_eq!(view, before);
}

#[test]
fn panning_moves_the_stage_and_reset_returns_to_fit() {
    let mut view = StageView::default();
    let home = view.image_rect(panel(), EXTENT);
    view.pan_by(Vec2::new(30.0, -10.0));
    assert_eq!(
        view.image_rect(panel(), EXTENT),
        home.translate(Vec2::new(30.0, -10.0))
    );
    view.pan_by(Vec2::new(f32::NAN, 0.0));
    assert_eq!(
        view.image_rect(panel(), EXTENT),
        home.translate(Vec2::new(30.0, -10.0))
    );
    view.zoom_center(2.0, panel(), EXTENT);
    assert!(!view.is_fit());
    view.reset();
    assert_eq!(view.image_rect(panel(), EXTENT), home);
}

#[test]
fn the_pasteboard_reaches_twice_the_longer_side_past_every_edge() {
    let board = pasteboard(EXTENT);
    let reach = EXTENT.x.max(EXTENT.y) * 2.0;
    assert_eq!(board.min, Pos2::new(-reach, -reach));
    assert_eq!(board.max, Pos2::new(EXTENT.x + reach, EXTENT.y + reach));
    assert_eq!(
        pasteboard(Vec2::new(0.0, 10.0)).area(),
        0.0,
        "a stage with no size has no pasteboard"
    );
}

#[test]
fn the_visible_pasteboard_is_the_whole_panel_in_stage_units_within_the_pasteboard() {
    // A panel wider than the stage: fitted, there is pasteboard to its left and right.
    let wide = Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(800.0, 200.0));
    let view = StageView::default();
    let stage = view.visible_region(wide, EXTENT).expect("stage");
    let board = view.visible_pasteboard(wide, EXTENT).expect("pasteboard");
    assert_eq!(stage, Rect::from_min_size(Pos2::ZERO, EXTENT));
    // Fitted at scale 1, the 400 unit stage sits in the middle of the 800 point panel, so
    // the panel reaches 200 units past each side of it.
    assert!(near(board.min, Pos2::new(-200.0, 0.0)), "{board:?}");
    assert!(near(board.max, Pos2::new(600.0, 200.0)), "{board:?}");
    // Zoomed all the way out, the panel shows more than the pasteboard: it stops there.
    let mut far = StageView::default();
    for _ in 0..40 {
        far.zoom_about(panel().center(), 1.0 / STEP, panel(), EXTENT);
    }
    assert_eq!(
        far.visible_pasteboard(panel(), EXTENT),
        Some(pasteboard(EXTENT))
    );
    assert!(
        StageView::default()
            .visible_pasteboard(panel(), Vec2::ZERO)
            .is_none()
    );
}

#[test]
fn the_visible_region_is_the_part_of_the_stage_inside_the_panel() {
    let mut view = StageView::default();
    // Fitted, the whole stage is visible.
    let all = view.visible_region(panel(), EXTENT).expect("region");
    assert_eq!((all.min, all.max), (Pos2::ZERO, Pos2::new(400.0, 200.0)));
    // Zoomed in 4x about the panel center: a quarter of each side is visible, centered.
    view.zoom_center(4.0, panel(), EXTENT);
    let r = view.visible_region(panel(), EXTENT).expect("region");
    assert!(
        (r.width() - 100.0).abs() < 0.01 && (r.height() - 50.0).abs() < 0.01,
        "{r:?}"
    );
    assert!(
        (r.center() - Pos2::new(200.0, 100.0)).length() < 0.01,
        "{r:?}"
    );
    // Panned far away, nothing is visible.
    view.pan_by(Vec2::new(1e6, 0.0));
    assert!(view.visible_region(panel(), EXTENT).is_none());
    // Partly off the panel: only the stage's left part shows, starting at its left edge.
    let mut view = StageView::default();
    view.pan_by(Vec2::new(400.0, 0.0));
    let r = view.visible_region(panel(), EXTENT).expect("region");
    assert_eq!(r.min.x, 0.0);
    assert!((r.max.x - 200.0).abs() < 0.01, "{r:?}");
    for bad in [Vec2::ZERO, Vec2::new(f32::NAN, 1.0)] {
        assert!(StageView::default().visible_region(panel(), bad).is_none());
    }
}

#[test]
fn zooming_before_the_panel_is_known_changes_nothing() {
    let mut view = StageView::default();
    view.zoom_center(2.0, Rect::NOTHING, EXTENT);
    view.zoom_about(Pos2::new(f32::NAN, 0.0), 2.0, panel(), EXTENT);
    assert!(view.is_fit(), "no NaN pan from an empty panel");
}

#[test]
fn degenerate_extents_and_panels_do_not_panic_or_vanish() {
    let mut view = StageView::default();
    for extent in [Vec2::ZERO, Vec2::new(f32::NAN, 5.0), Vec2::new(-1.0, 3.0)] {
        assert!(view.image_rect(panel(), extent).is_finite());
        view.zoom_center(2.0, panel(), extent);
        assert_eq!(view.zoom(), 1.0, "nothing to zoom");
    }
    let tiny = Rect::from_min_size(Pos2::ZERO, Vec2::ZERO);
    assert!(view.scale(tiny, EXTENT) >= MIN_SCALE);
}

#[test]
fn ruler_ticks_label_round_numbers_and_never_crowd() {
    // 1 point per unit, labels at least 50 points apart: every 50 units.
    let ticks = ruler_ticks(100.0, 1.0, 100.0, 400.0, 50.0);
    let majors: Vec<f32> = ticks.iter().filter(|t| t.major).map(|t| t.value).collect();
    assert_eq!(majors, vec![0.0, 50.0, 100.0, 150.0, 200.0, 250.0, 300.0]);
    // Zoomed in 10x, labels land on finer round numbers.
    let ticks = ruler_ticks(0.0, 10.0, 0.0, 200.0, 50.0);
    let majors: Vec<f32> = ticks.iter().filter(|t| t.major).map(|t| t.value).collect();
    assert_eq!(majors, vec![0.0, 5.0, 10.0, 15.0, 20.0]);
    for pair in ticks.windows(2) {
        assert!(
            pair[1].screen - pair[0].screen >= 3.0 - 1e-3,
            "minor ticks are not crowded"
        );
    }
    // A span that starts left of the origin still gets negative values.
    let ticks = ruler_ticks(100.0, 1.0, 0.0, 120.0, 50.0);
    assert!(ticks.iter().any(|t| t.major && t.value == -100.0));
}

#[test]
fn ruler_ticks_refuse_bad_numbers() {
    assert!(ruler_ticks(0.0, 0.0, 0.0, 100.0, 50.0).is_empty());
    assert!(ruler_ticks(f32::NAN, 1.0, 0.0, 100.0, 50.0).is_empty());
    assert!(
        ruler_ticks(0.0, 1.0, 100.0, 0.0, 50.0).is_empty(),
        "backwards span"
    );
    assert!(
        ruler_ticks(0.0, 1e-9, 0.0, 1e9, 50.0).len() < 3000,
        "bounded"
    );
}

fn turned(degrees: f32) -> StageView {
    let mut view = StageView::default();
    view.set_rotation_degrees(degrees);
    view
}

#[test]
fn a_turned_view_maps_screen_and_stage_both_ways() {
    for degrees in [0.0, 30.0, -90.0, 180.0, 137.0] {
        let mut view = turned(degrees);
        view.zoom_center(1.7, panel(), EXTENT);
        view.pan_by(Vec2::new(25.0, -40.0));
        for p in [Pos2::ZERO, Pos2::new(400.0, 200.0), Pos2::new(-30.0, 77.0)] {
            let back = view.to_stage(panel(), EXTENT, view.to_screen(panel(), EXTENT, p));
            assert!(near(back, p), "{degrees}: {p:?} came back as {back:?}");
        }
    }
}

#[test]
fn turning_a_quarter_puts_the_stage_top_on_its_right() {
    let view = turned(90.0);
    let center = panel().center();
    // The stage's top middle is above the center upright; a quarter turn clockwise moves it
    // to the right of the center.
    let top_middle = view.to_screen(panel(), EXTENT, Pos2::new(200.0, 0.0));
    assert!(
        near(top_middle, center + Vec2::new(200.0, 0.0)),
        "{top_middle:?}"
    );
    // The stage's center stays at the panel's.
    assert!(near(
        view.to_screen(panel(), EXTENT, Pos2::new(200.0, 100.0)),
        center
    ));
}

#[test]
fn panning_and_zooming_a_turned_view_follow_the_screen() {
    let mut view = turned(90.0);
    let p = Pos2::new(120.0, 40.0);
    let before = view.to_screen(panel(), EXTENT, p);
    view.pan_by(Vec2::new(30.0, 0.0));
    let after = view.to_screen(panel(), EXTENT, p);
    assert!(
        near(after, before + Vec2::new(30.0, 0.0)),
        "a drag right moves it right"
    );
    let anchor = Pos2::new(300.0, 180.0);
    let under = view.to_stage(panel(), EXTENT, anchor);
    view.zoom_about(anchor, 2.5, panel(), EXTENT);
    assert!(
        near(view.to_stage(panel(), EXTENT, anchor), under),
        "zoom keeps the point"
    );
}

#[test]
fn rotation_stays_in_one_turn_and_refuses_bad_angles() {
    let mut view = turned(370.0);
    assert!((view.rotation_degrees() - 10.0).abs() < 1e-3);
    view.rotate_by(f32::NAN);
    view.set_rotation(f32::INFINITY);
    assert!((view.rotation_degrees() - 10.0).abs() < 1e-3);
    view.set_rotation_degrees(-180.0);
    assert_eq!(view.rotation_degrees(), 180.0, "half a turn reads as 180");
    view.rotate_by(std::f32::consts::PI);
    assert!(!view.is_turned(), "a whole turn is upright again");
    let mut view = turned(45.0);
    view.zoom_center(2.0, panel(), EXTENT);
    view.reset();
    assert!(view.is_fit() && view.is_turned(), "fit keeps the turn");
    view.reset_rotation();
    assert_eq!(view, StageView::default());
}

#[test]
fn a_turned_view_shows_the_box_around_the_slanted_panel() {
    // Zoomed in on the center and turned 45 degrees, more of the stage is in view than the
    // panel's upright size, and the region stays centered.
    let mut view = StageView::default();
    view.zoom_center(4.0, panel(), EXTENT);
    let upright = view.visible_region(panel(), EXTENT).expect("upright");
    view.set_rotation_degrees(45.0);
    let slanted = view.visible_region(panel(), EXTENT).expect("turned");
    assert!(slanted.width() > upright.width() && slanted.height() > upright.height());
    assert!((slanted.center() - upright.center()).length() < 0.01);
}

#[test]
fn turns_between_points_and_snaps() {
    let c = Pos2::new(100.0, 100.0);
    let quarter =
        turn_between(c, c + Vec2::new(50.0, 0.0), c + Vec2::new(0.0, 50.0)).expect("turn");
    assert!(
        (quarter - std::f32::consts::FRAC_PI_2).abs() < 1e-5,
        "right to down is clockwise"
    );
    assert!(turn_between(c, c + Vec2::new(1.0, 0.0), c + Vec2::new(0.0, 50.0)).is_none());
    assert!(turn_between(c, Pos2::new(f32::NAN, 0.0), c + Vec2::X * 50.0).is_none());
    let snapped = snap_rotation(22.0_f32.to_radians(), 15.0).to_degrees();
    assert!((snapped - 15.0).abs() < 1e-3, "{snapped}");
    assert!((snap_rotation(23.0_f32.to_radians(), 15.0).to_degrees() - 30.0).abs() < 1e-3);
    assert_eq!(snap_rotation(0.3, 0.0), 0.3, "no step, no snap");
}
