#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::num::NonZeroU32;

use flare_commands::{Command, apply_all};
use flare_core::{BitmapId, DocumentId, FrameRate, InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{
    Bitmap, BitmapInstance, Content, Document, FrameSpan, Layer, Scene, Timeline,
};
use flare_tween::raster::FlowParams;

use super::bitmap::{BitmapInbetweenError, bitmap_inbetweens};
use crate::ops::SpanRef;

/// A 4x4 bitmap of one colour.
fn solid(fill: [u8; 4]) -> Bitmap {
    let pixels = fill.iter().copied().cycle().take(4 * 4 * 4).collect();
    Bitmap::new(BitmapId::new(), "b", 4, 4, pixels).expect("bitmap")
}

/// A document with a bitmap keyframe at frame 0 and at frame 4 on one layer.
fn document(first: &Bitmap, second: &Bitmap) -> (Document, SceneId, LayerId) {
    let (layer, scene) = (LayerId::new(), SceneId::new());
    let layer_value = Layer::new(layer, "L", vec![]).expect("layer");
    let empty = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(vec![layer_value]))],
        vec![],
    )
    .expect("document");
    // The library holds one copy of each distinct picture, so a repeated one is placed twice.
    let same = first.content_hash() == second.content_hash();
    let second_id = if same { first.id() } else { second.id() };
    let mut commands = vec![Command::AddBitmap(first.clone())];
    if !same {
        commands.push(Command::AddBitmap(second.clone()));
    }
    commands.push(place(layer, scene, 0, first.id()));
    commands.push(place(layer, scene, 4, second_id));
    let doc = apply_all(&empty, commands)
        .expect("applied")
        .expect("changed");
    (doc, scene, layer)
}

fn place(layer: LayerId, scene: SceneId, start: u32, bitmap: BitmapId) -> Command {
    let content = Content::Bitmap(BitmapInstance::new(
        InstanceId::new(),
        bitmap,
        Transform2D::IDENTITY,
    ));
    Command::InsertSpan {
        scene,
        layer,
        span: FrameSpan::new(start, NonZeroU32::new(2).expect("nz"), content),
    }
}

fn at(layer: LayerId, start: u32) -> SpanRef {
    SpanRef {
        layer,
        start,
        element: 0,
    }
}

#[test]
fn makes_the_requested_number_of_pictures_between_two_keyframes() {
    let (first, second) = (solid([255, 0, 0, 255]), solid([0, 0, 255, 255]));
    let (doc, scene, layer) = document(&first, &second);
    let out = bitmap_inbetweens(
        &doc,
        scene,
        at(layer, 0),
        at(layer, 4),
        3,
        FlowParams::default(),
    )
    .expect("frames");
    assert_eq!((out.width, out.height), (4, 4));
    assert_eq!(out.frames.len(), 3);
    assert!(out.frames.iter().all(|f| f.len() == 4 * 4 * 4));
}

#[test]
fn identical_keyframes_give_that_bitmap_back() {
    let same = solid([40, 90, 200, 255]);
    let (doc, scene, layer) = document(&same, &same);
    let out = bitmap_inbetweens(
        &doc,
        scene,
        at(layer, 0),
        at(layer, 4),
        2,
        FlowParams::default(),
    )
    .expect("frames");
    assert!(out.frames.iter().all(|f| f == same.pixels()));
}

#[test]
fn a_count_outside_its_bounds_is_refused() {
    let (first, second) = (solid([0; 4]), solid([0; 4]));
    let (doc, scene, layer) = document(&first, &second);
    for count in [0, 65] {
        let err = bitmap_inbetweens(
            &doc,
            scene,
            at(layer, 0),
            at(layer, 4),
            count,
            FlowParams::default(),
        )
        .unwrap_err();
        assert!(matches!(err, BitmapInbetweenError::Count(c) if c == count));
    }
}

#[test]
fn a_frame_without_a_keyframe_is_refused() {
    let (first, second) = (solid([0; 4]), solid([0; 4]));
    let (doc, scene, layer) = document(&first, &second);
    let err = bitmap_inbetweens(
        &doc,
        scene,
        at(layer, 0),
        at(layer, 2),
        1,
        FlowParams::default(),
    )
    .unwrap_err();
    assert!(matches!(err, BitmapInbetweenError::NoSuchKeyframe(2)));
}

#[test]
fn a_keyframe_without_a_bitmap_is_refused() {
    let (first, second) = (solid([0; 4]), solid([0; 4]));
    let (mut doc, scene, layer) = document(&first, &second);
    let empty = FrameSpan::new(8, NonZeroU32::new(2).expect("nz"), Content::Empty);
    doc = apply_all(
        &doc,
        [Command::InsertSpan {
            scene,
            layer,
            span: empty,
        }],
    )
    .expect("applied")
    .expect("changed");
    let err = bitmap_inbetweens(
        &doc,
        scene,
        at(layer, 0),
        at(layer, 8),
        1,
        FlowParams::default(),
    )
    .unwrap_err();
    assert!(matches!(err, BitmapInbetweenError::NotBitmap(8)));
}
