//! An armature's instances are published where each pose puts them, and stay the same objects
//! from pose to pose.

use flare_core::{DocumentPoint, InstanceId, Transform2D};
use flare_document::{Armature, Instance, Joint, JointPose, Tween};
use glamx::{DAffine2, DVec2};
use swf::PlaceObjectAction;

use super::*;

fn rigged() -> Document {
    let dot = flare_core::SymbolId::new();
    let symbol = flare_document::Symbol::new(
        dot,
        "Dot",
        Timeline::new(vec![layer(
            "L",
            vec![span(
                0,
                1,
                Content::Shape(rect(-6.0, -6.0, 12.0, 12.0, Rgba8::opaque(0, 0, 255))),
            )],
        )]),
    );
    let joint = |x: f64, parent| {
        let at = DAffine2::from_translation(DVec2::new(x, 40.0));
        Joint::new(
            Instance::new(
                InstanceId::new(),
                dot,
                Transform2D::new(at).expect("finite"),
            ),
            DocumentPoint::new(x, 40.0).expect("finite"),
            parent,
        )
    };
    let rest = Armature::new(vec![joint(30.0, None), joint(70.0, Some(0))]).expect("rig");
    let down = rest
        .with_pose(vec![
            JointPose {
                turn: std::f64::consts::FRAC_PI_4,
                shift: DVec2::ZERO,
            },
            JointPose::default(),
        ])
        .expect("pose");
    let arm = layer(
        "Armature",
        vec![
            span(0, 4, Content::Armature(rest)).with_tween(Tween::Linear),
            span(4, 4, Content::Armature(down)),
        ],
    );
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![arm]));
    document(120, 100, vec![scene], vec![symbol])
}

#[test]
fn every_frame_shows_the_pose_and_the_instances_are_only_moved() {
    let doc = rigged();
    let published = publish_swf(&doc, &PublishOptions::default()).expect("publish");
    assert_eq!(published.frames, 8);
    let parsed = parse(&published.bytes);
    let background = doc.stage().background();
    for frame in 0..8 {
        let (max, mean) = difference(
            &render_original(&doc, 0, frame),
            &render_swf_frame(&parsed, frame as usize, background),
        );
        assert!(
            max <= 60 && mean < 0.3,
            "frame {frame}: max {max}, mean {mean}"
        );
    }
    // Two instances are placed once each; every later frame only moves them, so a symbol with
    // frames of its own would keep playing through the poses.
    let placed = parsed
        .tags
        .iter()
        .filter(
            |t| matches!(t, Tag::PlaceObject(p) if matches!(p.action, PlaceObjectAction::Place(_))),
        )
        .count();
    assert_eq!(placed, 2);
}
