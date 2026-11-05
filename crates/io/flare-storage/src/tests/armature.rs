//! An armature layer's rig and every pose come back exactly; a damaged rig is refused.

use super::*;
use flare_core::{DocumentPoint, InstanceId};
use flare_document::{Armature, Instance, Joint, JointPose, PlayMode, Playback, Symbol, Tween};
use glamx::DVec2;

fn document() -> Document {
    let shape = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        3.0,
        3.0,
        Rgba8::new(0, 0, 255, 255),
    )
    .expect("rect");
    let art = Layer::new(
        LayerId::new(),
        "L",
        vec![FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape))],
    )
    .expect("layer");
    let symbol = Symbol::new(SymbolId::new(), "Part", Timeline::new(vec![art]));
    let instance = |x: f64| {
        Instance::new(
            InstanceId::new(),
            symbol.id(),
            Transform2D::new(DAffine2::from_scale_angle_translation(
                DVec2::new(1.5, 0.75),
                0.3,
                DVec2::new(x, 2.0),
            ))
            .expect("finite"),
        )
        .with_playback(Playback {
            mode: PlayMode::PlayOnce,
            first_frame: 2,
        })
    };
    let point = |x: f64| DocumentPoint::new(x, 1.0 / 3.0).expect("finite");
    let rest = Armature::new(vec![
        Joint::new(instance(0.0), point(0.0), None),
        Joint::new(instance(10.0), point(10.0), Some(0))
            .with_limit(Some((-0.7, 1.1)))
            .expect("limit"),
        Joint::new(instance(20.0), point(20.0), None),
    ])
    .expect("rig");
    let posed = rest
        .with_pose(vec![
            JointPose {
                turn: 0.123_456_789,
                shift: DVec2::new(-4.5, 1e-7),
            },
            JointPose {
                turn: -0.7,
                shift: DVec2::ZERO,
            },
            JointPose {
                turn: 3.0,
                shift: DVec2::new(1.0, 2.0),
            },
        ])
        .expect("pose");
    let spans = vec![
        FrameSpan::new(0, NonZeroU32::new(5).expect("5"), Content::Armature(rest))
            .with_tween(Tween::Linear)
            .with_ease(30),
        FrameSpan::new(5, NonZeroU32::new(5).expect("5"), Content::Armature(posed)),
    ];
    let layer = Layer::new(LayerId::new(), "Armature_1", spans).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![symbol],
    )
    .expect("document")
}

#[test]
fn every_pose_of_an_armature_survives_to_the_last_bit() {
    let document = document();
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("bones.flare");
    save(&document, &path).expect("save");
    let loaded = load(&path).expect("load");
    assert_eq!(
        loaded.scenes()[0].timeline(),
        document.scenes()[0].timeline()
    );
}
