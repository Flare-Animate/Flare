//! A file picker runs off the thread that asked, so the window never waits for it.
#![allow(clippy::expect_used)]

use std::sync::mpsc;
use std::time::Duration;

use flare_platform::jobs;

#[cfg(not(target_os = "macos"))]
#[test]
fn a_picker_blocks_its_own_thread_not_the_caller() {
    let (release, hold) = mpsc::channel::<()>();
    let (answered, answer) = mpsc::channel();
    let caller = std::thread::current().id();
    jobs::ask(
        move || {
            // The picker waits for the person, here for the test to let it go.
            hold.recv_timeout(Duration::from_secs(20)).ok()?;
            Some(std::thread::current().id())
        },
        move |picked| {
            let _ = answered.send(picked);
        },
    )
    .expect("starts");
    // `ask` returned while the picker still waits: the caller was free.
    assert!(answer.try_recv().is_err());
    release.send(()).expect("release");
    let picked = answer
        .recv_timeout(Duration::from_secs(20))
        .expect("answered")
        .expect("a pick");
    assert_ne!(picked, caller);
}

#[test]
fn a_cancelled_picker_answers_none() {
    let (answered, answer) = mpsc::channel();
    jobs::ask(
        || None::<u32>,
        move |picked| {
            let _ = answered.send(picked);
        },
    )
    .expect("starts");
    assert_eq!(
        answer
            .recv_timeout(Duration::from_secs(20))
            .expect("answered"),
        None
    );
}

#[test]
fn a_sprite_sheet_is_written_with_its_atlas_beside_it_and_a_scene_with_nothing_is_refused() {
    use flare_core::{DocumentId, FrameRate, LayerId, SceneId, ShapeId};
    use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};

    let shape =
        Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, flare_core::Rgba8::WHITE).expect("rect");
    let span = FrameSpan::new(
        0,
        std::num::NonZeroU32::new(4).expect("nonzero"),
        Content::Shape(shape),
    );
    let layer = Layer::new(LayerId::new(), "L", vec![span]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("sheet.png");
    jobs::write_sprites(&document, &path).expect("written");
    let png = std::fs::read(&path).expect("png");
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    let atlas = std::fs::read_to_string(dir.path().join("sheet.json")).expect("atlas");
    assert!(
        atlas.contains("sheet.png"),
        "the atlas names its picture: {atlas}"
    );

    let empty = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![],
        vec![],
    )
    .expect("document");
    let out = dir.path().join("none.png");
    assert!(jobs::write_sprites(&empty, &out).is_err());
    assert!(!out.exists(), "nothing is written for it");
}
