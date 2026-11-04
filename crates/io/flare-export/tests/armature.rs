//! XFL has no bones of Flare's: an armature goes out as the instances each pose puts in place,
//! and the export says so.
#![allow(clippy::expect_used)]

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, SceneId, SymbolId, Transform2D,
};
use flare_document::{
    Armature, Content, Document, FrameSpan, Instance, Joint, JointPose, Layer, Scene, Symbol,
    Timeline,
};
use flare_export::export_xfl;
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;

fn document() -> Document {
    let symbol = Symbol::new(SymbolId::new(), "Part", Timeline::default());
    let joint = |x: f64, parent| {
        let at = DAffine2::from_translation(DVec2::new(x, 0.0));
        Joint::new(
            Instance::new(
                InstanceId::new(),
                symbol.id(),
                Transform2D::new(at).expect("finite"),
            ),
            DocumentPoint::new(x, 0.0).expect("finite"),
            parent,
        )
    };
    let rest = Armature::new(vec![joint(0.0, None), joint(10.0, Some(0))]).expect("rig");
    let down = rest
        .with_pose(vec![
            JointPose {
                turn: std::f64::consts::FRAC_PI_2,
                shift: DVec2::ZERO,
            },
            JointPose::default(),
        ])
        .expect("pose");
    let spans = vec![
        FrameSpan::new(0, NonZeroU32::MIN, Content::Armature(rest)),
        FrameSpan::new(1, NonZeroU32::MIN, Content::Armature(down)),
    ];
    let layer = Layer::new(LayerId::new(), "Armature_1", spans).expect("layer");
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]))],
        vec![symbol],
    )
    .expect("document")
}

#[test]
fn each_pose_is_written_as_its_instances_where_it_puts_them() {
    let output = export_xfl(&document()).expect("export");
    let main = &output
        .files
        .iter()
        .find(|(n, _)| n.ends_with("DOMDocument.xml"))
        .expect("document")
        .1;
    assert_eq!(main.matches("<DOMSymbolInstance").count(), 4, "{main}");
    // In the second pose the second part has turned down to (0, 10).
    assert!(main.contains(r#"ty="10""#), "{main}");
    assert!(
        output
            .diagnostics
            .iter()
            .any(|n| n.message.contains("bones are not written")),
        "the export says the bones were left out"
    );
}
