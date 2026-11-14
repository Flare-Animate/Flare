#![allow(clippy::expect_used)]

use flare_core::{BitmapId, FrameRate, SceneId};
use flare_document::{Bitmap, Content, MAX_BITMAPS, MAX_TOTAL_BITMAP_BYTES, PlayMode};

use super::*;
use crate::test_support::{Fixture, add, fixture, rect_span};

fn rate(fps: u32) -> FrameRate {
    FrameRate::new(fps, 1).expect("rate")
}

/// A full HD movie at 30 frames a second, `seconds` long.
fn hd(seconds: Option<f64>) -> Source {
    Source {
        width: 1920,
        height: 1080,
        rate: rate(30),
        seconds,
    }
}

fn settings(fps: u32, max_seconds: f64, max_side: u32) -> Settings {
    Settings {
        rate: rate(fps),
        max_seconds,
        max_side,
        opacity: 40,
    }
}

#[test]
fn a_plan_counts_one_frame_per_picture_taken_and_says_how_long_they_last_at_the_document_rate() {
    let f = fixture(); // 24 frames a second
    let plan = plan(
        f.session.document(),
        &hd(Some(10.0)),
        &settings(12, 60.0, 960),
    )
    .expect("plan");
    assert_eq!(plan.frames, 120, "ten seconds at 12 a second");
    assert_eq!(plan.size, (960, 540), "scaled down in proportion");
    assert_eq!(plan.bytes, 120 * 960 * 540 * 4);
    assert!(plan.whole && plan.fits() && !plan.is_large());
    assert!(
        (plan.length_seconds - 5.0).abs() < 1e-9,
        "120 frames at 24 a second"
    );
    assert!((plan.speed - 2.0).abs() < 1e-9 && !plan.keeps_timing());

    let same = plan_at(&f, 24);
    assert!(same.keeps_timing(), "taken at the document's rate");
    assert!((same.length_seconds - 10.0).abs() < 1e-9);
}

fn plan_at(f: &Fixture, fps: u32) -> Plan {
    plan(
        f.session.document(),
        &hd(Some(10.0)),
        &settings(fps, 60.0, 960),
    )
    .expect("plan")
}

#[test]
fn the_most_minutes_cut_the_video_short_and_an_unknown_length_reads_up_to_them() {
    let f = fixture();
    let long = plan(
        f.session.document(),
        &hd(Some(100.0)),
        &settings(12, 30.0, 640),
    )
    .expect("plan");
    assert_eq!((long.frames, long.seconds, long.whole), (360, 30.0, false));
    let unknown = plan(f.session.document(), &hd(None), &settings(12, 30.0, 640)).expect("plan");
    assert_eq!((unknown.frames, unknown.whole), (360, false));
    let small = Source {
        width: 320,
        height: 240,
        ..hd(Some(1.0))
    };
    let kept = plan(f.session.document(), &small, &settings(12, 30.0, 960)).expect("plan");
    assert_eq!(kept.size, (320, 240), "a small video is never enlarged");
}

#[test]
fn a_video_too_large_for_the_document_says_so_and_how_long_or_how_small_would_fit() {
    let f = fixture();
    let document = f.session.document();
    let big = plan(document, &hd(Some(60.0)), &settings(24, 60.0, 1920)).expect("plan");
    let Some(TooLarge::Bytes { needed, room }) = big.too_large else {
        panic!("too many bytes: {:?}", big.too_large);
    };
    assert_eq!(
        (needed, room),
        (1440 * 1920 * 1080 * 4, MAX_TOTAL_BITMAP_BYTES)
    );
    let frame = 1920.0 * 1080.0 * 4.0;
    let expected = (MAX_TOTAL_BITMAP_BYTES as f64 / frame).floor() / 24.0;
    assert!(
        (big.fitting_seconds - expected).abs() < 1e-9,
        "{}",
        big.fitting_seconds
    );
    let shorter = plan(
        document,
        &hd(Some(60.0)),
        &settings(24, big.fitting_seconds, 1920),
    )
    .expect("plan");
    assert!(shorter.fits(), "the length it names fits");
    let side = big.fitting_side.expect("a smaller picture fits");
    let smaller = plan(document, &hd(Some(60.0)), &settings(24, 60.0, side)).expect("plan");
    assert!(
        smaller.fits() && smaller.is_large(),
        "the size it names fits"
    );
    let larger = plan(document, &hd(Some(60.0)), &settings(24, 60.0, side + 1)).expect("plan");
    assert!(!larger.fits(), "and is the largest that does");

    let many = plan(document, &hd(Some(60.0)), &settings(120, 60.0, 64)).expect("plan");
    assert_eq!(
        many.too_large,
        Some(TooLarge::Frames {
            needed: 7200,
            room: MAX_BITMAPS as u32
        })
    );
    assert_eq!(many.fitting_side, None, "no size fits that many pictures");
}

#[test]
fn the_suggested_settings_keep_the_document_rate_and_fit() {
    let f = fixture();
    let document = f.session.document();
    let long = hd(Some(600.0));
    let suggested = Settings::suggested(document, &long);
    assert_eq!(suggested.rate, rate(24));
    assert_eq!(
        (suggested.max_side, suggested.opacity),
        (DEFAULT_SIDE, DEFAULT_OPACITY)
    );
    let plan = plan(document, &long, &suggested).expect("plan");
    assert!(plan.fits() && plan.keeps_timing(), "{plan:?}");
    assert!(plan.seconds >= 1.0 && plan.seconds < DEFAULT_SECONDS);
    let short = Settings::suggested(document, &hd(Some(4.0)));
    assert!(
        (short.max_seconds - 4.0).abs() < 1e-9,
        "a short video is read whole"
    );
}

#[test]
fn settings_outside_their_ranges_are_refused() {
    let f = fixture();
    let document = f.session.document();
    for bad in [
        settings(24, 0.0, 960),
        settings(24, f64::NAN, 960),
        settings(24, MAX_VIDEO_SECONDS + 1.0, 960),
        settings(24, 10.0, MIN_VIDEO_SIDE - 1),
        settings(24, 10.0, MAX_VIDEO_SIDE + 1),
        Settings {
            opacity: 101,
            ..settings(24, 10.0, 960)
        },
    ] {
        assert!(
            matches!(
                plan(document, &hd(Some(5.0)), &bad),
                Err(VideoRefusal::Setting(_))
            ),
            "{bad:?}"
        );
    }
    let no_size = Source {
        width: 0,
        ..hd(Some(5.0))
    };
    assert_eq!(
        plan(document, &no_size, &settings(24, 10.0, 960)),
        Err(VideoRefusal::NoSize)
    );
}

/// A 4x2 picture of one grey level.
fn picture(level: u8) -> Vec<u8> {
    [level, level, level, 255].repeat(8)
}

fn small_plan(f: &Fixture) -> Plan {
    let source = Source {
        width: 4,
        height: 2,
        rate: rate(24),
        seconds: Some(1.0),
    };
    plan(f.session.document(), &source, &settings(24, 1.0, 960)).expect("plan")
}

#[test]
fn the_pictures_play_once_from_one_symbol_on_a_see_through_layer_at_the_bottom_in_one_undo_step() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 2, 0.0, 0.0, 10.0, 10.0));
    let plan = small_plan(&f);
    let pictures = vec![picture(10), picture(10), picture(200), picture(10)];
    let placed = place(f.session.document(), f.scene, 5, "clip", &plan, pictures).expect("placed");
    assert_eq!(
        (placed.frames, placed.distinct),
        (4, 2),
        "a picture that repeats is kept once"
    );
    f.session.apply(placed.commands.clone()).expect("applies");

    let document = f.session.document();
    let layers = document.scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 2);
    let layer = layers.last().expect("bottom");
    assert_eq!((layer.id(), layer.name()), (placed.layer, "clip"));
    assert_eq!(layer.view_opacity(), 40);
    assert!(!layer.is_locked() && layer.is_visible());
    let [span] = layer.spans() else {
        panic!("one keyframe")
    };
    assert_eq!(
        (span.start(), span.duration().get()),
        (5, 4),
        "a frame per picture"
    );
    let Content::Instance(instance) = span.content() else {
        panic!("an instance");
    };
    assert_eq!(instance.symbol(), placed.symbol);
    assert_eq!(instance.playback().mode, PlayMode::PlayOnce);
    let symbol = document.symbol(placed.symbol).expect("in the library");
    assert_eq!(symbol.name(), "clip");
    let starts: Vec<(u32, u32)> = symbol.timeline().layers()[0]
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get()))
        .collect();
    assert_eq!(starts, vec![(0, 2), (2, 1), (3, 1)], "the repeat is held");

    // Fitted to the 1920x1080 stage: a 4x2 picture is 480 times as large, 60 units down.
    let shown = |frame: u32| {
        let evaluated = flare_eval::evaluate_frame(document, f.scene, frame).expect("frame");
        evaluated
            .bitmaps
            .iter()
            .map(|b| (b.bitmap.pixels()[0], b.transform))
            .collect::<Vec<_>>()
    };
    let [(level, transform)] = shown(7)[..] else {
        panic!("one picture at frame 7");
    };
    assert_eq!(level, 200, "the third picture two frames in");
    assert!((transform.matrix2.x_axis.x - 480.0).abs() < 1e-9);
    assert!((transform.translation.y - 60.0).abs() < 1e-9);
    assert_eq!(shown(8)[0].0, 10);
    assert!(
        shown(4).is_empty() && shown(9).is_empty(),
        "only on its frames"
    );

    assert!(f.session.undo(), "one undo step");
    let document = f.session.document();
    assert_eq!(document.scenes()[0].timeline().layers().len(), 1);
    assert!(document.symbol(placed.symbol).is_none() && document.bitmaps().is_empty());
}

#[test]
fn a_second_import_of_the_same_video_reuses_its_pictures_under_a_new_symbol_name() {
    let mut f = fixture();
    let plan = small_plan(&f);
    let first = place(
        f.session.document(),
        f.scene,
        0,
        "clip",
        &plan,
        vec![picture(1)],
    )
    .expect("first");
    f.session.apply(first.commands).expect("applies");
    let again = place(
        f.session.document(),
        f.scene,
        0,
        "clip",
        &plan,
        vec![picture(1), picture(2)],
    )
    .expect("again");
    assert_eq!(
        again.distinct, 1,
        "the picture already in the library is shown"
    );
    f.session.apply(again.commands).expect("applies");
    let document = f.session.document();
    assert_eq!(document.bitmaps().len(), 2);
    assert_eq!(
        document.symbol(again.symbol).expect("symbol").name(),
        "clip 2"
    );
}

#[test]
fn wrong_pictures_and_a_missing_scene_are_refused() {
    let f = fixture();
    let plan = small_plan(&f);
    let document = f.session.document();
    assert_eq!(
        place(document, f.scene, 0, "clip", &plan, vec![]),
        Err(VideoRefusal::NoPictures)
    );
    assert_eq!(
        place(
            document,
            f.scene,
            0,
            "clip",
            &plan,
            vec![picture(1), vec![0; 3]]
        ),
        Err(VideoRefusal::WrongSize { index: 1 })
    );
    assert_eq!(
        place(document, SceneId::new(), 0, "clip", &plan, vec![picture(1)]),
        Err(VideoRefusal::NoSuchScene)
    );
}

#[test]
fn a_library_with_no_room_left_refuses_the_pictures_that_do_not_fit() {
    let f = fixture();
    // One 1x1 picture short of a full library.
    let bitmaps: Vec<Bitmap> = (0..MAX_BITMAPS - 1)
        .map(|n| {
            let [a, b, ..] = (n as u32).to_le_bytes();
            Bitmap::new(BitmapId::new(), "dot", 1, 1, vec![a, b, 0, 255]).expect("dot")
        })
        .collect();
    let document = f
        .session
        .document()
        .clone()
        .with_bitmaps(bitmaps)
        .expect("full");
    let plan = small_plan(&f);
    let placed = place(
        &document,
        f.scene,
        0,
        "clip",
        &plan,
        vec![picture(1), picture(2)],
    );
    assert_eq!(
        placed,
        Err(VideoRefusal::TooLarge(TooLarge::Frames {
            needed: 2,
            room: 1
        }))
    );
    assert!(
        place(
            &document,
            f.scene,
            0,
            "clip",
            &plan,
            vec![picture(1), picture(1)]
        )
        .is_ok()
    );
}

#[test]
fn the_video_has_the_box_of_its_pictures_and_a_point_on_them_picks_it() {
    let mut f = fixture();
    let plan = small_plan(&f);
    let placed = place(
        f.session.document(),
        f.scene,
        0,
        "clip",
        &plan,
        vec![picture(9)],
    )
    .expect("placed");
    f.session.apply(placed.commands.clone()).expect("applies");
    let document = f.session.document();
    // The 4x2 pictures fitted to the 1920x1080 stage: 1920 wide, 960 tall, 60 down.
    let bounds = crate::ops::span_bounds(document, f.scene, placed.at).expect("a box to transform");
    assert_eq!(
        (bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y),
        (0.0, 60.0, 1920.0, 1020.0)
    );
    let pick = |x: f64, y: f64| {
        crate::selection::picture_at(document, f.scene, 0, glamx::DVec2::new(x, y))
    };
    assert_eq!(pick(960.0, 540.0), Some(placed.at));
    assert_eq!(pick(960.0, 30.0), None, "above the pictures");
    let hidden = flare_commands::apply_all(
        document,
        [flare_commands::Command::UpdateLayer {
            scene: f.scene,
            layer: placed.layer,
            change: flare_commands::LayerChange::Visible(false),
        }],
    )
    .expect("hides")
    .expect("changed");
    assert_eq!(
        crate::selection::picture_at(&hidden, f.scene, 0, glamx::DVec2::new(960.0, 540.0)),
        None,
        "a hidden layer is not picked"
    );
}

#[test]
fn what_was_asked_is_laid_over_the_suggestion_and_the_plan_reads_as_json() {
    let f = fixture();
    let document = f.session.document();
    let asked = Asked {
        fps: Some(12.0),
        max_minutes: Some(0.5),
        ..Asked::default()
    };
    let chosen = asked
        .settings(document, &hd(Some(100.0)))
        .expect("settings");
    assert_eq!(chosen.rate, rate(12));
    assert!((chosen.max_seconds - 30.0).abs() < 1e-9);
    assert_eq!(chosen.max_side, DEFAULT_SIDE, "kept");
    let json = plan_json(&plan(document, &hd(Some(100.0)), &chosen).expect("plan"));
    assert_eq!(json["frames"], 360);
    assert_eq!(json["picture"]["width"], 960);
    assert_eq!(json["whole_video"], false);
    assert_eq!(json["speed"], 2.0);
    assert_eq!(json["fits"], true);
    assert!(json["too_large"].is_null());
    let bad = Asked {
        fps: Some(0.0),
        ..Asked::default()
    };
    assert!(matches!(
        bad.settings(document, &hd(Some(1.0))),
        Err(VideoRefusal::Setting(_))
    ));
    let big = plan(document, &hd(Some(60.0)), &settings(24, 60.0, 1920)).expect("plan");
    let json = plan_json(&big);
    assert_eq!(json["fits"], false);
    assert!(
        json["too_large"]["reason"]
            .as_str()
            .is_some_and(|r| r.contains("MB"))
    );
}
