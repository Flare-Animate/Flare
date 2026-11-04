//! What a document may say about armatures.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Armature, Content, Diagnostic, Document, FrameSpan, Instance, Joint, JointPose, Layer, Scene,
    Shape, Symbol, Timeline,
};
use glamx::DVec2;

fn symbol() -> Symbol {
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 4.0, 4.0, Rgba8::WHITE).expect("rect");
    let layer = Layer::new(
        LayerId::new(),
        "L",
        vec![FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape))],
    )
    .expect("layer");
    Symbol::new(SymbolId::new(), "Part", Timeline::new(vec![layer]))
}

fn rig(symbol: SymbolId) -> Armature {
    let joint = |x: f64, parent| {
        Joint::new(
            Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY),
            DocumentPoint::new(x, 0.0).expect("finite"),
            parent,
        )
    };
    Armature::new(vec![joint(0.0, None), joint(10.0, Some(0))]).expect("rig")
}

fn span(start: u32, content: Content) -> FrameSpan {
    FrameSpan::new(start, NonZeroU32::new(5).expect("nonzero"), content)
}

fn document(layers: Vec<Layer>, symbols: Vec<Symbol>) -> Result<Document, Vec<Diagnostic>> {
    Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(SceneId::new(), "S", Timeline::new(layers))],
        symbols,
    )
}

#[test]
fn poses_of_one_rig_share_their_instances() {
    let part = symbol();
    let rest = rig(part.id());
    let mut pose = rest.pose().to_vec();
    pose[0] = JointPose {
        turn: 1.0,
        shift: DVec2::new(3.0, 0.0),
    };
    let turned = rest.with_pose(pose).expect("pose");
    let layer = Layer::new(
        LayerId::new(),
        "Armature",
        vec![
            span(0, Content::Armature(rest)),
            span(5, Content::Armature(turned)),
        ],
    )
    .expect("layer");
    // The same instances in two keyframes are one rig, not duplicates.
    document(vec![layer], vec![part]).expect("valid");
}

#[test]
fn an_armature_layer_holds_one_rig_and_nothing_else() {
    let part = symbol();
    let one = rig(part.id());
    let other = rig(part.id());
    let layer = Layer::new(
        LayerId::new(),
        "A",
        vec![
            span(0, Content::Armature(one.clone())),
            span(5, Content::Armature(other)),
        ],
    )
    .expect("layer");
    let errors = document(vec![layer.clone()], vec![part.clone()]).expect_err("two rigs");
    assert!(errors.contains(&Diagnostic::ArmatureRigsDiffer(layer.id())));

    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 1.0, 1.0, Rgba8::WHITE).expect("rect");
    let mixed = Layer::new(
        LayerId::new(),
        "B",
        vec![
            span(0, Content::Armature(one.clone())),
            span(5, Content::Shape(shape)),
        ],
    )
    .expect("layer");
    let errors = document(vec![mixed.clone()], vec![part.clone()]).expect_err("mixed");
    assert!(errors.contains(&Diagnostic::ArmatureLayerMixed(mixed.id())));

    let mask = Layer::new(LayerId::new(), "M", vec![span(0, Content::Armature(one))])
        .expect("layer")
        .with_mask(true);
    let errors = document(vec![mask.clone()], vec![part]).expect_err("mask");
    assert!(errors.contains(&Diagnostic::ArmatureOnSpecialLayer(mask.id())));
}

#[test]
fn an_armature_instance_cannot_also_be_elsewhere_or_of_a_missing_symbol() {
    let part = symbol();
    let armature = rig(part.id());
    let stray = armature.joints()[1].instance().clone();
    let armature_layer = Layer::new(
        LayerId::new(),
        "A",
        vec![span(0, Content::Armature(armature))],
    )
    .expect("layer");
    let plain = Layer::new(
        LayerId::new(),
        "P",
        vec![span(0, Content::Instance(stray.clone()))],
    )
    .expect("layer");
    let errors = document(vec![armature_layer.clone(), plain], vec![part]).expect_err("twice");
    assert!(errors.contains(&Diagnostic::DuplicateInstance(stray.id())));

    let errors = document(vec![armature_layer], vec![]).expect_err("no symbol");
    assert!(
        errors
            .iter()
            .any(|e| matches!(e, Diagnostic::DanglingSymbol { .. }))
    );
}

#[test]
fn an_armature_is_never_one_of_several_elements() {
    let part = symbol();
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 1.0, 1.0, Rgba8::WHITE).expect("rect");
    assert!(
        span(0, Content::Armature(rig(part.id())))
            .with_extras(vec![Content::Shape(shape)])
            .is_err()
    );
}
