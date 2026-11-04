//! An armature places the instances it carries where its pose puts them, in every evaluation.
#![allow(clippy::expect_used)]

use std::f64::consts::FRAC_PI_2;
use std::num::NonZeroU32;

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Armature, Content, Document, FrameSpan, Instance, Joint, JointPose, Layer, Scene, Shape,
    Symbol, SymbolKind, Timeline, Tween,
};
use flare_eval::{LevelContent, evaluate_frame, evaluate_level};
use glamx::{DAffine2, DVec2};

/// A symbol of `frames` frames, a different square on each, so its clock can be read.
fn part(frames: u32) -> Symbol {
    let spans = (0..frames)
        .map(|f| {
            let square = Shape::rect(
                ShapeId::new(),
                f64::from(f),
                0.0,
                1.0,
                1.0,
                Rgba8::opaque(255, 0, 0),
            )
            .expect("rect");
            FrameSpan::new(f, NonZeroU32::MIN, Content::Shape(square))
        })
        .collect();
    let layer = Layer::new(LayerId::new(), "L", spans).expect("layer");
    Symbol::new(SymbolId::new(), "Part", Timeline::new(vec![layer]))
        .with_kind(SymbolKind::MovieClip)
}

/// A shoulder at the origin and a hand at (10, 0), each carrying an instance there.
fn arm(symbol: SymbolId) -> Armature {
    let joint = |x: f64, parent| {
        Joint::new(
            Instance::new(
                InstanceId::new(),
                symbol,
                Transform2D::new(DAffine2::from_translation(DVec2::new(x, 0.0))).expect("finite"),
            ),
            DocumentPoint::new(x, 0.0).expect("finite"),
            parent,
        )
    };
    Armature::new(vec![joint(0.0, None), joint(10.0, Some(0))]).expect("arm")
}

fn turned(rest: &Armature, turn: f64) -> Armature {
    rest.with_pose(vec![
        JointPose {
            turn,
            shift: DVec2::ZERO,
        },
        JointPose::default(),
    ])
    .expect("pose")
}

fn key(start: u32, armature: Armature, tween: Tween) -> FrameSpan {
    FrameSpan::new(
        start,
        NonZeroU32::new(4).expect("nonzero"),
        Content::Armature(armature),
    )
    .with_tween(tween)
}

fn document(keys: Vec<FrameSpan>, symbol: Symbol) -> (Document, SceneId) {
    let layer = Layer::new(LayerId::new(), "Armature", keys).expect("layer");
    let id = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(id, "S", Timeline::new(vec![layer]))],
        vec![symbol],
    )
    .expect("document");
    (document, id)
}

fn close(a: DVec2, b: DVec2) -> bool {
    (a - b).length() < 1e-9
}

#[test]
fn the_pose_places_each_instance_and_tweens_to_the_next() {
    let symbol = part(1);
    let rest = arm(symbol.id());
    let down = turned(&rest, FRAC_PI_2);
    let (document, scene) = document(
        vec![key(0, rest, Tween::Linear), key(4, down, Tween::None)],
        symbol,
    );
    let hand = |frame| {
        evaluate_frame(&document, scene, frame)
            .expect("eval")
            .placed[1]
            .transform
    };
    assert!(close(hand(0).translation, DVec2::new(10.0, 0.0)));
    assert!(close(hand(4).translation, DVec2::new(0.0, 10.0)));
    // Halfway the shoulder has turned an eighth of a turn.
    let half = std::f64::consts::FRAC_PI_4;
    assert!(close(
        hand(2).translation,
        DVec2::new(10.0 * half.cos(), 10.0 * half.sin())
    ));
}

#[test]
fn a_held_pose_does_not_tween_and_neither_do_two_rigs() {
    let symbol = part(1);
    let rest = arm(symbol.id());
    let down = turned(&rest, FRAC_PI_2);
    let (document, scene) = document(
        vec![key(0, rest.clone(), Tween::None), key(4, down, Tween::None)],
        symbol,
    );
    let at = evaluate_frame(&document, scene, 2).expect("eval").placed[1].transform;
    assert!(close(at.translation, DVec2::new(10.0, 0.0)));
}

#[test]
fn the_instances_keep_their_clock_from_one_pose_to_the_next() {
    let symbol = part(8);
    let rest = arm(symbol.id());
    let down = turned(&rest, FRAC_PI_2);
    let (document, scene) = document(
        vec![key(0, rest, Tween::None), key(4, down, Tween::None)],
        symbol,
    );
    // At frame 5 the symbol shows its frame 5, not frame 1 as a new instance would.
    let frame = evaluate_frame(&document, scene, 5).expect("eval");
    let first = frame.shapes.first().expect("a square");
    let x = left_edge(&first.shape);
    assert!((x - 5.0).abs() < 1e-9, "the square of frame 5, found {x}");

    let level = evaluate_level(&document, document.scenes()[0].timeline(), 5).expect("level");
    assert_eq!(level.items.len(), 2);
    for (index, item) in level.items.iter().enumerate() {
        assert_eq!(item.element, index);
        let LevelContent::Instance { span_start, .. } = item.content else {
            panic!("an instance");
        };
        assert_eq!(span_start, 0, "one run of poses is one instance");
    }
}

/// The left edge of a square drawn from its first point.
fn left_edge(shape: &Shape) -> f64 {
    match shape.path().first() {
        Some(flare_document::PathSegment::MoveTo(p)) => p.as_vec().x,
        _ => f64::NAN,
    }
}
