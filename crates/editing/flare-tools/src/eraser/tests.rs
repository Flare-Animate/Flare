#![allow(clippy::expect_used, unused_imports)]

use crate::ops::*;
use crate::test_support::*;
use emath::{Pos2, Rect};
use flare_commands::Command;
use flare_core::{
    DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId, Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Symbol, Timeline,
    Tween,
};
use flare_history::EditorSession;
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;

#[test]
fn deleting_removes_the_span() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 5, 0.0, 0.0, 5.0, 5.0));
    let commands = delete(f.session.document(), f.scene, at(&f, 0));
    f.session.apply(commands).expect("delete");
    assert!(find_span(f.session.document(), f.scene, at(&f, 0)).is_none());
}

/// (start, duration, path segments of the first element; 0 for nothing) of each span.
fn spans_with_paths(f: &Fixture) -> Vec<(u32, u32, usize)> {
    f.session.document().scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .map(|s| {
            let segments = match s.content() {
                Content::Shape(shape) => shape.path().len(),
                _ => 0,
            };
            (s.start(), s.duration().get(), segments)
        })
        .collect()
}

/// A square eraser outline from `(x0, y0)` to `(x1, y1)`.
fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<DVec2> {
    vec![
        DVec2::new(x0, y0),
        DVec2::new(x1, y0),
        DVec2::new(x1, y1),
        DVec2::new(x0, y1),
    ]
}

#[test]
fn erasing_on_a_held_frame_changes_that_frame_alone() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 6, 0.0, 0.0, 40.0, 10.0));
    let whole = spans_with_paths(&f)[0].2;
    // Through the middle of the bar, on frame 2 of the keyframe at 0.
    let eraser = [square(15.0, -5.0, 25.0, 15.0)];
    let commands = crate::eraser::erase(f.session.document(), f.scene, 2, &eraser, None);
    f.session.apply(commands).expect("erase");
    let spans = spans_with_paths(&f);
    assert_eq!(
        spans.iter().map(|s| (s.0, s.1)).collect::<Vec<_>>(),
        vec![(0, 2), (2, 1), (3, 3)],
        "the frame became a keyframe of its own between the two parts"
    );
    assert_eq!(
        (spans[0].2, spans[2].2),
        (whole, whole),
        "the frames around it are whole"
    );
    assert!(spans[1].2 > whole, "the bar on frame 2 is cut in two");
    assert_eq!(
        f.session.undo_labels().count(),
        2,
        "the bar, then one erase step"
    );

    // On the keyframe's own first frame the keyframe is what is erased.
    let commands = crate::eraser::erase(f.session.document(), f.scene, 3, &eraser, None);
    f.session.apply(commands).expect("erase the keyframe");
    let spans = spans_with_paths(&f);
    assert_eq!(spans.len(), 3, "no further split");
    assert!(spans[2].2 > whole);
}

#[test]
fn the_faucet_on_a_held_frame_takes_the_shape_off_that_frame_alone() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 3, 0.0, 0.0, 40.0, 10.0));
    let commands = delete_on_frame(f.session.document(), f.scene, at(&f, 0), 1);
    f.session.apply(commands).expect("faucet");
    let spans = spans_with_paths(&f);
    assert_eq!(
        spans,
        vec![(0, 1, 5), (1, 1, 0), (2, 1, 5)],
        "frame 1 is a blank keyframe; frames 0 and 2 keep the bar"
    );
    let commands = delete_on_frame(f.session.document(), f.scene, at(&f, 0), 0);
    f.session.apply(commands).expect("faucet on the keyframe");
    assert_eq!(
        spans_with_paths(&f)[0].0,
        1,
        "the keyframe at 0 had only the bar"
    );
    assert!(
        delete_on_frame(f.session.document(), f.scene, at(&f, 7), 7).is_empty(),
        "nothing there"
    );
}

#[test]
fn an_eraser_kept_to_a_layer_leaves_the_other_layers_alone() {
    let (scene, folder, inner, other) = (
        SceneId::new(),
        LayerId::new(),
        LayerId::new(),
        LayerId::new(),
    );
    let layers = vec![
        Layer::new(folder, "Folder", vec![])
            .expect("folder")
            .with_folder(true),
        Layer::new(inner, "Inner", vec![rect_span(0, 1, 0.0, 0.0, 40.0, 10.0)])
            .expect("inner")
            .with_parent(Some(folder)),
        Layer::new(other, "Other", vec![rect_span(0, 1, 0.0, 0.0, 40.0, 10.0)]).expect("other"),
    ];
    let rate = FrameRate::new(24, 1).expect("rate");
    let scene_value = Scene::new(scene, "S", Timeline::new(layers));
    let document =
        Document::new(DocumentId::new(), rate, vec![scene_value], vec![]).expect("document");
    let eraser = [square(15.0, -5.0, 25.0, 15.0)];
    let touched = |only: Option<LayerId>| -> Vec<LayerId> {
        let mut layers: Vec<LayerId> = crate::eraser::erase(&document, scene, 0, &eraser, only)
            .iter()
            .filter_map(|c| match c {
                Command::InsertSpan { layer, .. } => Some(*layer),
                _ => None,
            })
            .collect();
        layers.dedup();
        layers
    };
    assert_eq!(
        touched(None),
        vec![inner, other],
        "with none chosen, every layer"
    );
    assert_eq!(touched(Some(other)), vec![other], "the chosen layer alone");
    assert_eq!(touched(Some(inner)), vec![inner]);
    assert_eq!(
        touched(Some(folder)),
        vec![inner],
        "a chosen folder reaches the layers in it"
    );
    let timeline = document.scenes()[0].timeline().layers();
    assert!(!crate::eraser::reaches(timeline, 9, None), "no such layer");
}
