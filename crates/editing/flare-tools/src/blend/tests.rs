#![allow(clippy::expect_used)]

use super::*;
use crate::test_support::{Fixture, add, at, fixture};
use flare_core::{InstanceId, Transform2D};
use flare_document::FrameSpan;
use flare_input::PointerKind;
use std::num::NonZeroU32;

const RED: [u8; 4] = [255, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

/// A `w` by `h` bitmap whose pixel `(x, y)` is `color(x, y)`.
fn picture(w: u32, h: u32, color: impl Fn(u32, u32) -> [u8; 4]) -> Bitmap {
    let rgba = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .flat_map(|(x, y)| color(x, y))
        .collect();
    Bitmap::new(BitmapId::new(), "picture", w, h, rgba).expect("bitmap")
}

/// Black on the left half, white on the right.
fn edge(w: u32, h: u32) -> Bitmap {
    picture(w, h, |x, _| if x < w / 2 { BLACK } else { WHITE })
}

/// Puts `bitmap` in the library and on the fixture's layer at frames 0 to 4, through `matrix`.
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
    let five = NonZeroU32::new(5).expect("five");
    add(f, FrameSpan::new(0, five, Content::Bitmap(placed)));
    id
}

fn placed(f: &mut Fixture, bitmap: Bitmap) -> BitmapId {
    place(f, bitmap, DAffine2::IDENTITY)
}

fn line(points: &[(f64, f64)]) -> Vec<InputSample> {
    points
        .iter()
        .map(|(x, y)| InputSample::mouse(*x, *y).expect("finite"))
        .collect()
}

fn settings(kind: BlendKind, size: f32) -> BlendSettings {
    BlendSettings {
        size,
        ..BlendSettings::for_kind(kind)
    }
}

/// Rubs `points` with `kind` and applies the result.
fn rub_and_apply(f: &mut Fixture, kind: BlendKind, settings: BlendSettings, points: &[(f64, f64)]) {
    let edit = rub(
        f.session.document(),
        f.scene,
        0,
        kind,
        settings,
        &line(points),
    )
    .expect("rub");
    f.session.apply(edit.commands).expect("apply");
}

fn shown(f: &Fixture) -> Bitmap {
    match find_element(f.session.document(), f.scene, at(f, 0)) {
        Some(Content::Bitmap(placed)) => f
            .session
            .document()
            .bitmap(placed.bitmap())
            .expect("in the library")
            .clone(),
        other => panic!("not a bitmap: {other:?}"),
    }
}

fn pixel(bitmap: &Bitmap, x: u32, y: u32) -> [u8; 4] {
    let i = ((y * bitmap.width() + x) * 4) as usize;
    let p = &bitmap.pixels()[i..i + 4];
    [p[0], p[1], p[2], p[3]]
}

#[test]
fn blur_softens_a_hard_edge_where_it_passes_and_nowhere_else() {
    let mut f = fixture();
    placed(&mut f, edge(64, 64));
    let blur = settings(BlendKind::Blur, 16.0);
    rub_and_apply(&mut f, BlendKind::Blur, blur, &[(32.0, 4.0), (32.0, 30.0)]);
    let after = shown(&f);
    let (dark, light) = (pixel(&after, 31, 16), pixel(&after, 32, 16));
    assert!(
        dark[0] > 20,
        "the black side of the edge got lighter: {dark:?}"
    );
    assert!(light[0] < 235, "the white side got darker: {light:?}");
    assert_eq!(pixel(&after, 31, 60), BLACK, "far below the stroke");
    assert_eq!(pixel(&after, 32, 60), WHITE);
    assert_eq!(
        pixel(&after, 2, 16),
        BLACK,
        "far from the edge, blur has nothing to mix"
    );
}

#[test]
fn going_over_the_same_place_in_one_stroke_does_not_blur_it_further() {
    let mut once = fixture();
    let mut thrice = fixture();
    placed(&mut once, edge(64, 32));
    placed(&mut thrice, edge(64, 32));
    let half = BlendSettings {
        strength: 0.5,
        ..settings(BlendKind::Blur, 16.0)
    };
    let pass = [(32.0, 4.0), (32.0, 28.0)];
    let back_and_forth = [(32.0, 4.0), (32.0, 28.0), (32.0, 4.0), (32.0, 28.0)];
    rub_and_apply(&mut once, BlendKind::Blur, half, &pass);
    rub_and_apply(&mut thrice, BlendKind::Blur, half, &back_and_forth);
    let (a, b) = (shown(&once), shown(&thrice));
    assert_eq!(pixel(&a, 31, 16), pixel(&b, 31, 16));
    assert_eq!(pixel(&a, 32, 16), pixel(&b, 32, 16));
    // A second stroke does go further.
    rub_and_apply(&mut once, BlendKind::Blur, half, &pass);
    assert!(pixel(&shown(&once), 31, 16)[0] > pixel(&a, 31, 16)[0]);
}

#[test]
fn blurring_into_clear_pixels_keeps_the_color_and_fades_the_alpha() {
    let mut f = fixture();
    placed(
        &mut f,
        picture(64, 32, |x, _| if x < 32 { RED } else { CLEAR }),
    );
    rub_and_apply(
        &mut f,
        BlendKind::Blur,
        settings(BlendKind::Blur, 16.0),
        &[(32.0, 4.0), (32.0, 28.0)],
    );
    let after = shown(&f);
    let faded = pixel(&after, 33, 16);
    assert!(faded[3] > 0 && faded[3] < 255, "{faded:?}");
    assert_eq!(&faded[..3], &[255, 0, 0], "no dark fringe");
}

/// A red block on the left of a white picture, smudged from the red across the white.
fn smudged(stretch: u8) -> Bitmap {
    let mut f = fixture();
    placed(
        &mut f,
        picture(96, 24, |x, _| if x < 16 { RED } else { WHITE }),
    );
    let smudge = BlendSettings {
        stretch,
        ..settings(BlendKind::Smudge, 12.0)
    };
    rub_and_apply(
        &mut f,
        BlendKind::Smudge,
        smudge,
        &[(6.0, 12.0), (90.0, 12.0)],
    );
    shown(&f)
}

/// How much redder than white a pixel is.
fn red_in(p: [u8; 4]) -> i32 {
    i32::from(p[0]) - i32::from(p[1])
}

#[test]
fn smudge_drags_color_along_the_stroke_and_stretch_carries_it_further() {
    let long = smudged(70);
    assert!(
        red_in(pixel(&long, 30, 12)) > 60,
        "{:?}",
        pixel(&long, 30, 12)
    );
    assert_eq!(pixel(&long, 30, 0), WHITE, "above the stroke");
    let short = smudged(0);
    assert!(
        red_in(pixel(&long, 50, 12)) > red_in(pixel(&short, 50, 12)),
        "{:?} {:?}",
        pixel(&long, 50, 12),
        pixel(&short, 50, 12)
    );
}

#[test]
fn a_smudge_click_has_picked_up_nothing_to_lay_down() {
    let mut f = fixture();
    placed(&mut f, edge(32, 32));
    let result = rub(
        f.session.document(),
        f.scene,
        0,
        BlendKind::Smudge,
        settings(BlendKind::Smudge, 10.0),
        &line(&[(16.0, 16.0)]),
    );
    assert_eq!(result, Err(BlendError::NothingChanged));
}

#[test]
fn light_pressure_makes_a_narrower_stroke() {
    let width = |pressure: f32| {
        let mut f = fixture();
        placed(&mut f, edge(64, 64));
        let points: Vec<InputSample> = [(32.0, 4.0), (32.0, 60.0)]
            .iter()
            .map(|(x, y)| InputSample::new(*x, *y, pressure, PointerKind::Pen).expect("sample"))
            .collect();
        let doc = f.session.document();
        let edit = rub(
            doc,
            f.scene,
            0,
            BlendKind::Blur,
            settings(BlendKind::Blur, 24.0),
            &points,
        )
        .expect("rub");
        f.session.apply(edit.commands).expect("apply");
        let after = shown(&f);
        (0..64)
            .filter(|&x| pixel(&after, x, 32) != pixel(&edge(64, 64), x, 32))
            .count()
    };
    assert!(width(0.3) < width(1.0), "{} {}", width(0.3), width(1.0));
}

#[test]
fn size_is_in_stage_units_whatever_the_bitmap_is_scaled_by() {
    let mut f = fixture();
    // Drawn ten times its size: a 40-unit dab is 4 pixels of the bitmap.
    let ten = DAffine2::from_scale(DVec2::splat(10.0));
    place(&mut f, edge(64, 64), ten);
    let blur = BlendSettings {
        hardness: 1.0,
        ..settings(BlendKind::Blur, 40.0)
    };
    rub_and_apply(
        &mut f,
        BlendKind::Blur,
        blur,
        &[(320.0, 40.0), (320.0, 600.0)],
    );
    let after = shown(&f);
    assert_ne!(pixel(&after, 31, 30), BLACK);
    assert_eq!(
        pixel(&after, 26, 30),
        BLACK,
        "five pixels off the line is out of reach"
    );
}

#[test]
fn the_edit_shows_a_new_bitmap_drops_the_old_one_and_undoes_in_one_step() {
    let mut f = fixture();
    let old = placed(&mut f, edge(32, 32));
    rub_and_apply(
        &mut f,
        BlendKind::Blur,
        settings(BlendKind::Blur, 12.0),
        &[(16.0, 2.0), (16.0, 30.0)],
    );
    let new = shown(&f).id();
    assert_ne!(new, old);
    assert!(
        f.session.document().bitmap(old).is_none(),
        "nothing else shows it"
    );
    assert!(f.session.undo());
    assert_eq!(shown(&f).id(), old);
}

#[test]
fn a_picture_shown_elsewhere_stays_in_the_library() {
    let mut f = fixture();
    let old = placed(&mut f, edge(32, 32));
    let again = BitmapInstance::new(
        InstanceId::new(),
        old,
        Transform2D::new(DAffine2::IDENTITY).expect("matrix"),
    );
    let one = NonZeroU32::new(1).expect("one");
    add(&mut f, FrameSpan::new(10, one, Content::Bitmap(again)));
    rub_and_apply(
        &mut f,
        BlendKind::Blur,
        settings(BlendKind::Blur, 12.0),
        &[(16.0, 2.0), (16.0, 30.0)],
    );
    assert!(f.session.document().bitmap(old).is_some());
}

#[test]
fn a_stroke_applied_as_it_goes_keeps_one_picture_in_the_library() {
    let mut f = fixture();
    placed(&mut f, edge(48, 48));
    let doc = f.session.document();
    let start = Pos2::new(24.0, 4.0);
    let blur = settings(BlendKind::Blur, 12.0);
    let mut stroke =
        BlendStroke::begin(doc, f.scene, 0, start, BlendKind::Blur, blur).expect("begin");
    for points in [&[(24.0, 4.0), (24.0, 20.0)][..], &[(24.0, 40.0)][..]] {
        stroke.extend(&line(points));
        assert!(stroke.changed());
        let edit = stroke.edit(f.session.document(), f.scene).expect("edit");
        f.session.apply(edit.commands.clone()).expect("apply");
        stroke.shows(edit.bitmap);
        assert!(!stroke.changed());
    }
    assert_eq!(f.session.document().bitmaps().len(), 1);
    let after = shown(&f);
    assert_ne!(
        pixel(&after, 23, 36),
        BLACK,
        "the second stretch was laid too"
    );
}

#[test]
fn a_stroke_whose_bitmap_was_replaced_meanwhile_is_stale() {
    let mut f = fixture();
    placed(&mut f, edge(32, 32));
    let doc = f.session.document();
    let blur = settings(BlendKind::Blur, 12.0);
    let start = Pos2::new(16.0, 4.0);
    let mut stroke =
        BlendStroke::begin(doc, f.scene, 0, start, BlendKind::Blur, blur).expect("begin");
    stroke.extend(&line(&[(16.0, 4.0), (16.0, 28.0)]));
    // Something else changes the picture first.
    rub_and_apply(&mut f, BlendKind::Blur, blur, &[(16.0, 4.0), (16.0, 28.0)]);
    assert_eq!(
        stroke.edit(f.session.document(), f.scene),
        Err(BlendError::Stale)
    );
}

#[test]
fn a_point_far_off_the_bitmap_is_walked_to_without_laying_dabs_there() {
    let mut f = fixture();
    placed(&mut f, edge(32, 32));
    let doc = f.session.document();
    let points = line(&[(16.0, 16.0), (1.0e12, 16.0), (16.0, 20.0)]);
    let edit = rub(
        doc,
        f.scene,
        0,
        BlendKind::Blur,
        settings(BlendKind::Blur, 12.0),
        &points,
    )
    .expect("finishes, and blurs where it is on the bitmap");
    assert!(!edit.commands.is_empty());
}

#[test]
fn rub_refuses_what_it_cannot_do() {
    let mut f = fixture();
    placed(&mut f, edge(32, 32));
    let doc = f.session.document();
    let ok = settings(BlendKind::Blur, 12.0);
    let attempt = |settings: BlendSettings, points: &[(f64, f64)]| {
        rub(doc, f.scene, 0, BlendKind::Blur, settings, &line(points))
    };
    let on = [(16.0, 16.0), (20.0, 16.0)];
    assert_eq!(attempt(ok, &[]), Err(BlendError::NoPoints));
    assert_eq!(attempt(ok, &[(500.0, 500.0)]), Err(BlendError::NoBitmap));
    let size = BlendSettings { size: 0.0, ..ok };
    assert_eq!(attempt(size, &on), Err(BlendError::Size));
    let strength = BlendSettings {
        strength: 1.5,
        ..ok
    };
    assert_eq!(attempt(strength, &on), Err(BlendError::Strength));
    let hardness = BlendSettings {
        hardness: -0.1,
        ..ok
    };
    assert_eq!(attempt(hardness, &on), Err(BlendError::Hardness));
    let stretch = BlendSettings { stretch: 101, ..ok };
    assert_eq!(attempt(stretch, &on), Err(BlendError::Stretch));
    let nothing = BlendSettings {
        strength: 0.0,
        ..ok
    };
    assert_eq!(attempt(nothing, &on), Err(BlendError::NothingChanged));
    let many = vec![(16.0, 16.0); MAX_POINTS + 1];
    assert_eq!(attempt(ok, &many), Err(BlendError::TooManyPoints));
}

#[test]
fn a_bitmap_squashed_flat_cannot_be_stroked() {
    let mut f = fixture();
    let flat = DAffine2::from_cols_array(&[1.0, 0.0, 1.0, 0.0, 0.0, 0.0]);
    place(&mut f, edge(32, 32), flat);
    // The wand finds no pixel under a flat picture either, so nothing is there to stroke.
    let result = rub(
        f.session.document(),
        f.scene,
        0,
        BlendKind::Blur,
        settings(BlendKind::Blur, 12.0),
        &line(&[(16.0, 0.0), (20.0, 0.0)]),
    );
    assert!(matches!(
        result,
        Err(BlendError::NoBitmap | BlendError::Flat)
    ));
}

#[test]
fn kinds_read_their_names_back() {
    for kind in BlendKind::ALL {
        assert_eq!(BlendKind::parse(kind.name()), Some(kind));
        assert_eq!(BlendKind::parse(&kind.name().to_uppercase()), Some(kind));
    }
    assert_eq!(BlendKind::parse("finger"), None);
}
