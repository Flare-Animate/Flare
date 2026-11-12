#![allow(clippy::expect_used)]

use super::*;
use crate::test_support::{Fixture, add, at, fixture, rect_span};
use flare_core::{InstanceId, Transform2D};
use flare_document::FrameSpan;
use glamx::DAffine2;
use std::num::NonZeroU32;

const RED: [u8; 4] = [255, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];

/// A bitmap from rows of pixels.
fn bitmap(rows: &[&[[u8; 4]]]) -> Bitmap {
    let (w, h) = (rows[0].len() as u32, rows.len() as u32);
    let rgba = rows
        .iter()
        .flat_map(|r| r.iter().flatten().copied())
        .collect();
    Bitmap::new(BitmapId::new(), "picture", w, h, rgba).expect("bitmap")
}

/// Red on the left, blue on the right, and one red pixel alone in the blue.
fn two_halves() -> Bitmap {
    bitmap(&[
        &[RED, RED, BLUE, BLUE],
        &[RED, RED, BLUE, RED],
        &[RED, RED, BLUE, BLUE],
    ])
}

/// Puts `bitmap` in the library and on the fixture's layer at frame 0 through `matrix`.
fn place(f: &mut Fixture, bitmap: Bitmap, matrix: DAffine2) -> BitmapId {
    let id = bitmap.id();
    f.session
        .apply([Command::AddBitmap(bitmap)])
        .expect("add bitmap");
    let placed = BitmapInstance::new(
        InstanceId::new(),
        id,
        Transform2D::new(matrix).expect("matrix"),
    );
    add(
        f,
        FrameSpan::new(
            0,
            NonZeroU32::new(5).expect("five"),
            Content::Bitmap(placed),
        ),
    );
    id
}

fn placed_bitmap(f: &Fixture) -> BitmapId {
    match find_element(f.session.document(), f.scene, at(f, 0)) {
        Some(Content::Bitmap(placed)) => placed.bitmap(),
        other => panic!("not a bitmap: {other:?}"),
    }
}

#[test]
fn a_contiguous_pick_stays_inside_the_joined_area_and_a_global_one_takes_every_like_pixel() {
    let picture = two_halves();
    let joined = pick(&picture, 0, 0, 0, true).expect("inside");
    assert_eq!(joined.count(), 6);
    assert!(joined.contains(1, 2) && !joined.contains(3, 1));
    let everywhere = pick(&picture, 0, 0, 0, false).expect("inside");
    assert_eq!(everywhere.count(), 7);
    assert!(everywhere.contains(3, 1));
    assert!(
        pick(&picture, 4, 0, 0, true).is_none(),
        "outside the picture"
    );
}

#[test]
fn the_threshold_is_how_far_each_channel_may_be_from_the_clicked_color() {
    let near = [250, 5, 0, 255];
    let picture = bitmap(&[&[RED, near]]);
    assert_eq!(pick(&picture, 0, 0, 5, true).expect("in").count(), 2);
    assert_eq!(pick(&picture, 0, 0, 4, true).expect("in").count(), 1);
}

#[test]
fn every_clear_pixel_is_the_same_color_whatever_its_hidden_rgb() {
    let picture = bitmap(&[&[[255, 0, 0, 0], [0, 255, 0, 0], RED]]);
    let clear = pick(&picture, 0, 0, 0, true).expect("in");
    assert_eq!(clear.count(), 2);
    assert!(!clear.contains(2, 0));
}

#[test]
fn masks_add_subtract_and_invert_and_ignore_a_mask_of_another_size() {
    let picture = two_halves();
    let mut red = pick(&picture, 0, 0, 0, true).expect("in");
    let blue = pick(&picture, 2, 0, 0, true).expect("in");
    red.add(&blue);
    assert_eq!(red.count(), 11);
    red.subtract(&blue);
    assert_eq!(red.count(), 6);
    red.invert();
    assert_eq!(red.count(), 6);
    assert!(red.contains(3, 1) && !red.contains(0, 0));
    let before = red.clone();
    red.add(&Mask::empty(2, 2));
    red.subtract(&Mask::empty(2, 2));
    assert_eq!(red, before);
}

#[test]
fn coverage_clears_the_picked_pixels_and_fades_the_kept_edge_by_the_feather() {
    let mut mask = Mask::empty(6, 1);
    mask.add(
        &pick(
            &bitmap(&[&[RED, BLUE, BLUE, BLUE, BLUE, BLUE]]),
            0,
            0,
            0,
            true,
        )
        .expect("in"),
    );
    assert_eq!(coverage(&mask, 0.0), [0.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
    let soft = coverage(&mask, 2.0);
    assert_eq!(soft[0], 0.0);
    assert!((soft[1] - 0.25).abs() < 1e-6, "{soft:?}");
    assert!((soft[2] - 0.75).abs() < 1e-6, "{soft:?}");
    assert_eq!(soft[3..], [1.0, 1.0, 1.0]);
}

#[test]
fn smoothing_names_round_trip_and_soften_more_in_order() {
    for s in Smoothing::ALL {
        assert_eq!(Smoothing::parse(s.name()), Some(s));
    }
    assert_eq!(Smoothing::parse(" Smooth "), Some(Smoothing::Smooth));
    assert_eq!(Smoothing::parse("blurry"), None);
    assert!(
        Smoothing::ALL
            .windows(2)
            .all(|p| p[0].feather() < p[1].feather())
    );
}

#[test]
fn the_bitmap_under_a_point_is_found_through_its_transform() {
    let mut f = fixture();
    let matrix = DAffine2::from_scale_angle_translation(
        glamx::DVec2::splat(10.0),
        0.0,
        glamx::DVec2::new(100.0, 50.0),
    );
    let id = place(&mut f, two_halves(), matrix);
    let doc = f.session.document();
    let hit = bitmap_at(doc, f.scene, 2, Pos2::new(135.0, 61.0)).expect("on the bitmap");
    assert_eq!((hit.x, hit.y, hit.bitmap, hit.at), (3, 1, id, at(&f, 0)));
    assert!(bitmap_at(doc, f.scene, 2, Pos2::new(99.0, 61.0)).is_none());
    assert!(bitmap_at(doc, f.scene, 2, Pos2::new(140.0, 61.0)).is_none());
    assert!(
        bitmap_at(doc, f.scene, 9, Pos2::new(135.0, 61.0)).is_none(),
        "no span there"
    );
}

#[test]
fn a_hidden_or_locked_layer_and_a_shape_are_not_picked_from() {
    let mut f = fixture();
    place(&mut f, two_halves(), DAffine2::IDENTITY);
    let point = Pos2::new(1.0, 1.0);
    assert!(bitmap_at(f.session.document(), f.scene, 0, point).is_some());
    for change in [
        flare_commands::LayerChange::Visible(false),
        flare_commands::LayerChange::Locked(true),
    ] {
        f.session
            .apply([Command::UpdateLayer {
                scene: f.scene,
                layer: f.layer,
                change,
            }])
            .expect("layer");
        assert!(bitmap_at(f.session.document(), f.scene, 0, point).is_none());
        assert!(f.session.undo());
    }
    let mut g = fixture();
    add(&mut g, rect_span(0, 1, 0.0, 0.0, 10.0, 10.0));
    assert_eq!(
        click(
            g.session.document(),
            g.scene,
            0,
            point,
            &WandSettings::default(),
            Combine::Replace,
            None
        ),
        Err(WandError::NoBitmap)
    );
}

#[test]
fn clicks_replace_add_to_and_take_from_the_picked_pixels() {
    let mut f = fixture();
    place(&mut f, two_halves(), DAffine2::IDENTITY);
    let doc = f.session.document().clone();
    let exact = WandSettings {
        threshold: 0,
        ..WandSettings::default()
    };
    let go = |p: (f32, f32), how, current| {
        click(&doc, f.scene, 0, Pos2::new(p.0, p.1), &exact, how, current).expect("click")
    };
    let red = go((0.5, 0.5), Combine::Replace, None).expect("picked");
    assert_eq!(red.mask.count(), 6);
    let both = go((2.5, 0.5), Combine::Add, Some(red.clone())).expect("picked");
    assert_eq!(both.mask.count(), 11);
    let blue = go((0.5, 0.5), Combine::Subtract, Some(both)).expect("picked");
    assert_eq!(blue.mask.count(), 5);
    assert_eq!(
        go((2.5, 0.5), Combine::Subtract, Some(blue)),
        None,
        "nothing left"
    );
    assert_eq!(go((2.5, 0.5), Combine::Subtract, None), None);
    assert_eq!(
        go((2.5, 0.5), Combine::Replace, Some(red))
            .expect("picked")
            .mask
            .count(),
        5
    );
}

#[test]
fn clearing_makes_the_picked_pixels_transparent_in_one_undo_step_and_drops_the_old_picture() {
    let mut f = fixture();
    let old = place(&mut f, two_halves(), DAffine2::IDENTITY);
    let doc = f.session.document().clone();
    let settings = WandSettings {
        threshold: 0,
        ..WandSettings::default()
    };
    let picked = click(
        &doc,
        f.scene,
        0,
        Pos2::new(3.0, 0.5),
        &settings,
        Combine::Replace,
        None,
    )
    .expect("click")
    .expect("picked");
    let commands = clear(&doc, f.scene, &picked, Smoothing::Pixels).expect("clear");
    f.session.apply(commands).expect("apply");
    let now = placed_bitmap(&f);
    assert_ne!(now, old);
    let document = f.session.document();
    assert!(document.bitmap(old).is_none(), "nothing else showed it");
    let pixels = document.bitmap(now).expect("new").pixels();
    let alpha = |x: usize, y: usize| pixels[(y * 4 + x) * 4 + 3];
    assert_eq!(
        (alpha(2, 0), alpha(3, 2), alpha(3, 1), alpha(0, 0)),
        (0, 0, 255, 255)
    );
    assert!(f.session.undo());
    assert_eq!(
        placed_bitmap(&f),
        old,
        "one step back is the picture as it was"
    );
}

#[test]
fn a_picture_other_elements_show_stays_in_the_library_and_keeps_its_pixels() {
    let mut f = fixture();
    let old = place(&mut f, two_halves(), DAffine2::IDENTITY);
    let again = BitmapInstance::new(InstanceId::new(), old, Transform2D::IDENTITY);
    add(
        &mut f,
        FrameSpan::new(5, NonZeroU32::new(1).expect("one"), Content::Bitmap(again)),
    );
    let doc = f.session.document().clone();
    let mask = pick(doc.bitmap(old).expect("old"), 0, 0, 0, true).expect("in");
    let picked = WandSelection {
        at: at(&f, 0),
        bitmap: old,
        mask,
    };
    let commands = clear(&doc, f.scene, &picked, Smoothing::Normal).expect("clear");
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::RemoveBitmap(_)))
    );
    f.session.apply(commands).expect("apply");
    assert_eq!(f.session.document().bitmap(old), doc.bitmap(old));
}

#[test]
fn a_stale_empty_or_already_clear_selection_changes_nothing() {
    let mut f = fixture();
    let clear_left = bitmap(&[&[[0, 0, 0, 0], RED]]);
    let id = place(&mut f, clear_left, DAffine2::IDENTITY);
    let doc = f.session.document().clone();
    let bits = doc.bitmap(id).expect("bitmap");
    let mut selection = WandSelection {
        at: at(&f, 0),
        bitmap: id,
        mask: pick(bits, 0, 0, 0, true).expect("in"),
    };
    assert_eq!(
        clear(&doc, f.scene, &selection, Smoothing::Pixels),
        Err(WandError::NothingToClear)
    );
    selection.mask = Mask::empty(2, 1);
    assert_eq!(
        clear(&doc, f.scene, &selection, Smoothing::Pixels),
        Err(WandError::NothingPicked)
    );
    selection.mask = Mask::empty(3, 1);
    assert_eq!(
        clear(&doc, f.scene, &selection, Smoothing::Pixels),
        Err(WandError::Stale)
    );
    selection.mask = Mask::empty(2, 1);
    selection.bitmap = BitmapId::new();
    assert_eq!(
        clear(&doc, f.scene, &selection, Smoothing::Pixels),
        Err(WandError::Stale)
    );
    let mut g = fixture();
    add(&mut g, rect_span(0, 1, 0.0, 0.0, 10.0, 10.0));
    selection.at = at(&g, 0);
    assert_eq!(
        clear(g.session.document(), g.scene, &selection, Smoothing::Pixels),
        Err(WandError::NotBitmap)
    );
}

#[test]
fn clearing_to_pixels_already_in_the_library_reuses_that_picture() {
    let mut f = fixture();
    let blank = bitmap(&[&[[0, 0, 0, 0], RED]]);
    let blank_id = blank.id();
    f.session.apply([Command::AddBitmap(blank)]).expect("add");
    let old = place(&mut f, bitmap(&[&[BLUE, RED]]), DAffine2::IDENTITY);
    let doc = f.session.document().clone();
    let picked = WandSelection {
        at: at(&f, 0),
        bitmap: old,
        mask: pick(doc.bitmap(old).expect("old"), 0, 0, 0, true).expect("in"),
    };
    let commands = clear(&doc, f.scene, &picked, Smoothing::Pixels).expect("clear");
    assert!(!commands.iter().any(|c| matches!(c, Command::AddBitmap(_))));
    f.session.apply(commands).expect("apply");
    assert_eq!(placed_bitmap(&f), blank_id);
}

#[test]
fn a_preview_is_at_most_the_side_asked_for_and_keeps_where_the_pixels_are() {
    let mut mask = Mask::empty(400, 100);
    mask.add(&pick(&bitmap(&[&[RED; 400]; 100].map(|r| &r[..])), 0, 0, 0, true).expect("in"));
    let small = mask.preview(40);
    assert_eq!(
        (small.width(), small.height(), small.count()),
        (40, 10, 400)
    );
    let mut left = Mask::empty(400, 100);
    left.add(&mask);
    left.subtract(&pick(&bitmap(&[&[RED; 400]; 100].map(|r| &r[..])), 0, 0, 0, true).expect("in"));
    assert!(left.preview(40).is_empty());
    assert_eq!(mask.preview(1000), mask, "a small mask is itself");
}

#[test]
fn points_pick_together_on_one_bitmap_and_refuse_two_bitmaps_none_or_a_bad_threshold() {
    let mut f = fixture();
    place(&mut f, two_halves(), DAffine2::IDENTITY);
    let other = BitmapInstance::new(
        InstanceId::new(),
        place_only(&mut f, bitmap(&[&[RED]])),
        Transform2D::new(DAffine2::from_translation(glamx::DVec2::new(50.0, 0.0))).expect("matrix"),
    );
    let span = crate::ops::find_span(f.session.document(), f.scene, at(&f, 0))
        .expect("span")
        .with_appended(Content::Bitmap(other))
        .expect("room");
    f.session
        .apply(crate::common::replace_span(f.scene, at(&f, 0), span))
        .expect("second bitmap");
    let doc = f.session.document();
    let exact = WandSettings {
        threshold: 0,
        ..WandSettings::default()
    };
    let points = |list: &[(f32, f32)]| list.iter().map(|p| Pos2::new(p.0, p.1)).collect::<Vec<_>>();
    let both = pick_points(
        doc,
        f.scene,
        0,
        &points(&[(0.5, 0.5), (2.5, 0.5)]),
        &exact,
        false,
    )
    .expect("picked");
    assert_eq!(both.mask.count(), 11);
    let rest = pick_points(doc, f.scene, 0, &points(&[(0.5, 0.5)]), &exact, true).expect("picked");
    assert_eq!(rest.mask.count(), 6);
    assert_eq!(
        pick_points(
            doc,
            f.scene,
            0,
            &points(&[(0.5, 0.5), (50.5, 0.5)]),
            &exact,
            false
        ),
        Err(WandError::DifferentBitmaps)
    );
    assert_eq!(
        pick_points(doc, f.scene, 0, &[], &exact, false),
        Err(WandError::NoPoints)
    );
    let many = vec![Pos2::new(0.5, 0.5); MAX_POINTS + 1];
    assert_eq!(
        pick_points(doc, f.scene, 0, &many, &exact, false),
        Err(WandError::TooManyPoints)
    );
    let wild = WandSettings {
        threshold: 201,
        ..exact
    };
    assert_eq!(
        pick_points(doc, f.scene, 0, &points(&[(0.5, 0.5)]), &wild, false),
        Err(WandError::Threshold)
    );
}

/// Puts `bitmap` in the library only.
fn place_only(f: &mut Fixture, bitmap: Bitmap) -> BitmapId {
    let id = bitmap.id();
    f.session
        .apply([Command::AddBitmap(bitmap)])
        .expect("add bitmap");
    id
}
