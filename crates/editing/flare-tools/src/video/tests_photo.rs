#![allow(clippy::expect_used)]

//! Video pictures kept as photos (JPEG) or, when asked, lossless.

use flare_commands::Command;
use flare_document::Bitmap;

use super::super::*;
use super::{fixture, hd, picture, rate, settings, small_plan};

/// A smooth 96x64 picture, as a frame of video is, brightened by `level`.
fn frame(level: u8) -> Vec<u8> {
    (0..64u32)
        .flat_map(|y| {
            (0..96u32).flat_map(move |x| [(x * 2) as u8 / 2 + level / 2, (y * 3) as u8, 80, 255])
        })
        .collect()
}

#[test]
fn video_pictures_are_kept_as_photos_unless_asked_to_stay_lossless() {
    let f = fixture();
    let document = f.session.document();
    let source = Source {
        width: 96,
        height: 64,
        rate: rate(24),
        seconds: Some(1.0),
    };
    let photo_plan = plan(document, &source, &settings(24, 1.0, 960)).expect("plan");
    let pictures = vec![frame(0), frame(0), frame(90)];
    let placed =
        place(document, f.scene, 0, "clip", &photo_plan, pictures.clone()).expect("placed");
    assert_eq!(placed.distinct, 2, "a repeated picture is still kept once");
    let added: Vec<&Bitmap> = placed
        .commands
        .iter()
        .filter_map(|c| match c {
            Command::AddBitmap(b) => Some(b),
            _ => None,
        })
        .collect();
    assert!(
        added
            .iter()
            .all(|b| b.photo().map(|p| p.quality()) == Some(flare_document::DEFAULT_PHOTO_QUALITY))
    );
    let lossless = Plan {
        settings: Settings {
            photo_quality: None,
            ..photo_plan.settings
        },
        ..photo_plan
    };
    let placed = place(document, f.scene, 0, "clip", &lossless, pictures).expect("placed");
    assert!(
        placed
            .commands
            .iter()
            .all(|c| !matches!(c, Command::AddBitmap(b) if b.photo().is_some()))
    );
    // A picture too small for a JPEG to help (the 4x2 ones above) simply stays lossless.
    let tiny = place(
        document,
        f.scene,
        0,
        "clip",
        &small_plan(&f),
        vec![picture(5)],
    )
    .expect("placed");
    assert!(
        tiny.commands
            .iter()
            .any(|c| matches!(c, Command::AddBitmap(b) if b.photo().is_none()))
    );
}

#[test]
fn the_photo_quality_can_be_asked_for_or_turned_off() {
    let f = fixture();
    let document = f.session.document();
    let source = hd(Some(5.0));
    let quality = Asked {
        quality: Some(70),
        ..Asked::default()
    };
    assert_eq!(
        quality
            .settings(document, &source)
            .expect("ok")
            .photo_quality,
        Some(70)
    );
    let exact = Asked {
        lossless: true,
        ..Asked::default()
    };
    assert_eq!(
        exact.settings(document, &source).expect("ok").photo_quality,
        None
    );
    for bad in [0, 101] {
        let asked = Asked {
            quality: Some(bad),
            ..Asked::default()
        };
        assert!(matches!(
            asked.settings(document, &source),
            Err(VideoRefusal::Setting(_))
        ));
    }
    let json = plan_json(
        &plan(
            document,
            &source,
            &quality.settings(document, &source).expect("ok"),
        )
        .expect("plan"),
    );
    assert_eq!(json["photo_quality"], 70);
}
