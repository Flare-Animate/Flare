#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_commands::{Command, EditError, apply_all};
use flare_core::{BitmapId, DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Diagnostic, Document, FrameSpan, Layer, Scene, Timeline,
};

fn bitmap(fill: u8) -> Bitmap {
    Bitmap::new(BitmapId::new(), "b", 2, 2, vec![fill; 16]).expect("bitmap")
}

fn document(layer: LayerId, scene: SceneId) -> Document {
    let layer = Layer::new(layer, "L", vec![]).expect("layer");
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(vec![layer]))],
        vec![],
    )
    .expect("document")
}

fn place(layer: LayerId, scene: SceneId, id: BitmapId) -> Command {
    let content = Content::Bitmap(BitmapInstance::new(
        InstanceId::new(),
        id,
        Transform2D::IDENTITY,
    ));
    Command::InsertSpan {
        scene,
        layer,
        span: FrameSpan::new(0, NonZeroU32::new(3).expect("nz"), content),
    }
}

#[test]
fn add_then_place_in_one_batch() {
    let (l, s) = (LayerId::new(), SceneId::new());
    let b = bitmap(7);
    let after = apply_all(
        &document(l, s),
        [Command::AddBitmap(b.clone()), place(l, s, b.id())],
    )
    .expect("applied")
    .expect("changed");
    assert_eq!(after.bitmaps().len(), 1);
}

#[test]
fn placing_a_missing_bitmap_fails_and_changes_nothing() {
    let (l, s) = (LayerId::new(), SceneId::new());
    let before = document(l, s);
    let err = apply_all(&before, [place(l, s, BitmapId::new())]).expect_err("dangling");
    assert!(
        matches!(&err, EditError::Invalid(d) if matches!(d[0], Diagnostic::DanglingBitmap { .. }))
    );
    assert!(before.bitmaps().is_empty());
}

#[test]
fn identical_pixels_are_refused() {
    let (l, s) = (LayerId::new(), SceneId::new());
    let b = bitmap(1);
    let with = apply_all(&document(l, s), [Command::AddBitmap(b.clone())])
        .expect("ok")
        .expect("changed");
    let err = apply_all(&with, [Command::AddBitmap(bitmap(1))]).expect_err("dup");
    assert_eq!(err, EditError::DuplicateBitmap(b.id()));
}

#[test]
fn removal_needs_a_known_bitmap_with_no_users() {
    let (l, s) = (LayerId::new(), SceneId::new());
    let b = bitmap(2);
    let used = apply_all(
        &document(l, s),
        [Command::AddBitmap(b.clone()), place(l, s, b.id())],
    )
    .expect("ok")
    .expect("changed");
    let unknown = BitmapId::new();
    assert_eq!(
        apply_all(&used, [Command::RemoveBitmap(unknown)]).expect_err("none"),
        EditError::NoSuchBitmap(unknown)
    );
    assert!(matches!(
        apply_all(&used, [Command::RemoveBitmap(b.id())]).expect_err("in use"),
        EditError::Invalid(_)
    ));
}
