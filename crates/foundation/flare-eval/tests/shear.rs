//! Motion tweens between keyframes whose matrices have shear or reflection, as imported FLA
//! files have them: the frame must draw, never fail.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{DocumentId, FrameRate, InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, Scene, Symbol, Timeline, Tween,
};
use flare_eval::evaluate_frame;
use glamx::DAffine2;

fn scene(from: DAffine2, to: DAffine2) -> (Document, SceneId) {
    let art = SymbolId::new();
    let place = |start, duration, m| {
        let t = Transform2D::new(m).expect("finite");
        FrameSpan::new(
            start,
            NonZeroU32::new(duration).expect("nonzero"),
            Content::Instance(Instance::new(InstanceId::new(), art, t)),
        )
    };
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![
            place(0, 10, from).with_tween(Tween::Linear),
            place(10, 5, to),
        ],
    )
    .expect("layer");
    let id = SceneId::new();
    let doc = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(id, "S", Timeline::new(vec![layer]))],
        vec![Symbol::new(art, "Art", Timeline::default())],
    )
    .expect("document");
    (doc, id)
}

#[test]
fn sheared_and_reflected_keyframes_tween_and_render() {
    let sheared = DAffine2::from_cols_array(&[1.0, 0.2, 0.7, 1.0, 5.0, 5.0]);
    let reflected = DAffine2::from_cols_array(&[-2.0, 0.5, 0.5, 2.0, 50.0, 0.0]);
    let (doc, id) = scene(sheared, reflected);
    for frame in 0..15 {
        let f = evaluate_frame(&doc, id, frame).expect("frame");
        assert!(f.warnings.is_empty(), "frame {frame}: {:?}", f.warnings);
        assert_eq!(f.placed.len(), 1);
        assert!(f.placed[0].transform.is_finite());
    }
    let at = |frame| evaluate_frame(&doc, id, frame).expect("frame").placed[0].transform;
    assert_eq!(at(0), sheared);
    assert_eq!(at(10), reflected);
    assert_ne!(at(5), sheared);
}

#[test]
fn equal_sheared_keyframes_hold_the_identical_matrix() {
    let m = DAffine2::from_cols_array(&[1.0, 0.3, 0.9, -1.0, 7.0, 2.0]);
    let (doc, id) = scene(m, m);
    let f = evaluate_frame(&doc, id, 4).expect("frame");
    assert_eq!(f.placed[0].transform, m);
}
