#![allow(clippy::expect_used)]

use std::f64::consts::FRAC_PI_2;
use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{
    DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId,
    Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, Scene, Shape, Symbol, Timeline, Tween,
};
use flare_history::EditorSession;
use glamx::{DAffine2, DVec2};

use super::*;

struct Rig {
    session: EditorSession,
    scene: SceneId,
    plain: LayerId,
    /// Three instances along the x axis, at 0, 10 and 20, on `plain` for 10 frames.
    parts: [InstanceId; 3],
}

fn instance(symbol: SymbolId, x: f64) -> Instance {
    Instance::new(
        InstanceId::new(),
        symbol,
        Transform2D::new(DAffine2::from_translation(DVec2::new(x, 0.0))).expect("finite"),
    )
}

fn rig() -> Rig {
    let shape = Shape::rect(ShapeId::new(), -1.0, -1.0, 2.0, 2.0, Rgba8::WHITE).expect("rect");
    let art = Layer::new(
        LayerId::new(),
        "L",
        vec![FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape))],
    )
    .expect("layer");
    let symbol = Symbol::new(SymbolId::new(), "Part", Timeline::new(vec![art]));
    let parts = [0.0, 10.0, 20.0].map(|x| instance(symbol.id(), x));
    let ids = [parts[0].id(), parts[1].id(), parts[2].id()];
    let mut elements = parts.into_iter().map(Content::Instance);
    let first = elements.next().expect("one");
    let span = FrameSpan::new(0, NonZeroU32::new(10).expect("ten"), first)
        .with_extras(elements.collect())
        .expect("elements");
    let plain = LayerId::new();
    let layer = Layer::new(plain, "Parts", vec![span]).expect("layer");
    let scene = SceneId::new();
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(scene, "S", Timeline::new(vec![layer]))],
        vec![symbol],
    )
    .expect("document");
    Rig {
        session: EditorSession::new(document),
        scene,
        plain,
        parts: ids,
    }
}

fn bond(instance: InstanceId) -> Bond {
    Bond { instance, at: None }
}

fn layers(r: &Rig) -> &[Layer] {
    r.session.document().scenes()[0].timeline().layers()
}

/// Links the first two parts and returns the armature layer.
fn linked(r: &mut Rig) -> LayerId {
    let commands = connect(
        r.session.document(),
        r.scene,
        0,
        bond(r.parts[0]),
        bond(r.parts[1]),
        "Armature_1",
    )
    .expect("connect");
    r.session.apply(commands).expect("valid");
    layers(r)[0].id()
}

fn placed(r: &Rig, frame: u32) -> Vec<(InstanceId, DVec2)> {
    flare_eval::evaluate(r.session.document(), r.scene, frame)
        .expect("eval")
        .into_iter()
        .filter(|p| p.path.len() == 1)
        .map(|p| (p.path[0], p.transform.translation))
        .collect()
}

fn where_is(r: &Rig, frame: u32, id: InstanceId) -> DVec2 {
    placed(r, frame)
        .into_iter()
        .find(|(i, _)| *i == id)
        .map(|(_, at)| at)
        .expect("placed")
}

fn close(a: DVec2, b: DVec2) -> bool {
    a.distance(b) < 1e-6
}

#[test]
fn a_bone_between_two_instances_makes_an_armature_layer_and_moves_nothing() {
    let mut r = rig();
    let before = placed(&r, 3);
    let armature = linked(&mut r);
    assert_eq!(layers(&r).len(), 2);
    assert_eq!(
        layers(&r)[0].name(),
        "Armature_1",
        "above the layer they were on"
    );
    let span = &layers(&r)[0].spans()[0];
    assert_eq!((span.start(), span.duration().get()), (0, 10));
    assert_eq!(span.tween(), Tween::Linear, "poses tween");
    let Content::Armature(a) = span.content() else {
        panic!("an armature");
    };
    assert_eq!(a.joints().len(), 2);
    assert_eq!(a.joints()[1].parent(), Some(0));
    // The third part stays where it was, alone.
    assert_eq!(layers(&r)[1].spans()[0].element_count(), 1);
    let mut after = placed(&r, 3);
    let mut before = before;
    after.sort_by_key(|(id, _)| *id.as_uuid());
    before.sort_by_key(|(id, _)| *id.as_uuid());
    assert_eq!(after, before);
    assert!(
        joint_of(r.session.document(), r.scene, 3, r.parts[1]).is_some_and(|j| j
            == JointRef {
                layer: armature,
                joint: 1
            })
    );
    // It was one step.
    assert!(r.session.undo());
    assert_eq!(layers(&r).len(), 1);
}

#[test]
fn dragging_a_joint_bends_the_chain_toward_the_pointer() {
    let mut r = rig();
    let layer = linked(&mut r);
    let commands = connect(
        r.session.document(),
        r.scene,
        0,
        bond(r.parts[1]),
        bond(r.parts[2]),
        "unused",
    )
    .expect("grow");
    r.session.apply(commands).expect("valid");
    // The hand is 20 from the shoulder; pull it to (10, 10), which the arm can reach.
    let commands = pose_joint(
        r.session.document(),
        r.scene,
        layer,
        0,
        2,
        DVec2::new(10.0, 10.0),
    )
    .expect("pose");
    r.session.apply(commands).expect("valid");
    assert!(where_is(&r, 0, r.parts[2]).distance(DVec2::new(10.0, 10.0)) < 0.01);
    assert!(
        close(where_is(&r, 0, r.parts[0]), DVec2::ZERO),
        "the root stays"
    );
    // Out of reach, the arm points straight at it.
    let commands = pose_joint(
        r.session.document(),
        r.scene,
        layer,
        0,
        2,
        DVec2::new(0.0, 100.0),
    )
    .expect("pose");
    r.session.apply(commands).expect("valid");
    assert!(where_is(&r, 0, r.parts[2]).distance(DVec2::new(0.0, 20.0)) < 0.05);
}

#[test]
fn a_pose_inside_a_keyframe_splits_it_and_the_first_part_tweens_into_it() {
    let mut r = rig();
    let layer = linked(&mut r);
    let commands = turn_joint(r.session.document(), r.scene, layer, 6, 0, FRAC_PI_2).expect("turn");
    r.session.apply(commands).expect("valid");
    let spans: Vec<(u32, u32, Tween)> = layers(&r)[0]
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get(), s.tween()))
        .collect();
    assert_eq!(spans, vec![(0, 6, Tween::Linear), (6, 4, Tween::Linear)]);
    assert!(close(where_is(&r, 6, r.parts[1]), DVec2::new(0.0, 10.0)));
    assert!(close(where_is(&r, 0, r.parts[1]), DVec2::new(10.0, 0.0)));
    let half = where_is(&r, 3, r.parts[1]);
    assert!(
        close(half, DVec2::from_angle(FRAC_PI_2 / 2.0) * 10.0),
        "{half}"
    );
    // On the keyframe's own frame the pose is replaced, not split again.
    let commands = turn_joint(r.session.document(), r.scene, layer, 6, 0, 0.0).expect("turn");
    r.session.apply(commands).expect("valid");
    assert_eq!(layers(&r)[0].spans().len(), 2);
}

#[test]
fn dragging_the_root_moves_the_whole_armature() {
    let mut r = rig();
    let layer = linked(&mut r);
    let commands = pose_joint(
        r.session.document(),
        r.scene,
        layer,
        0,
        0,
        DVec2::new(5.0, 5.0),
    )
    .expect("move");
    r.session.apply(commands).expect("valid");
    assert!(close(where_is(&r, 0, r.parts[0]), DVec2::new(5.0, 5.0)));
    assert!(close(where_is(&r, 0, r.parts[1]), DVec2::new(15.0, 5.0)));
}

#[test]
fn a_limit_holds_a_joint_in_every_pose_and_in_the_solver() {
    let mut r = rig();
    let layer = linked(&mut r);
    let commands =
        set_limit(r.session.document(), r.scene, layer, 0, Some((-0.25, 0.25))).expect("limit");
    r.session.apply(commands).expect("valid");
    let commands = turn_joint(r.session.document(), r.scene, layer, 0, 0, FRAC_PI_2).expect("turn");
    r.session.apply(commands).expect("valid");
    let (posed, _) = armature_at(r.session.document(), r.scene, layer, 0).expect("armature");
    assert_eq!(posed.pose()[0].turn, 0.25);
    let pulled = reach(&posed, 1, DVec2::new(0.0, 10.0));
    assert!(pulled.pose()[0].turn <= 0.25 + 1e-12);
    assert!(
        set_limit(r.session.document(), r.scene, layer, 0, Some((1.0, -1.0))).is_err(),
        "a limit backwards"
    );
}

#[test]
fn detaching_and_releasing_keep_the_instances_where_they_are() {
    let mut r = rig();
    let layer = linked(&mut r);
    let commands = turn_joint(r.session.document(), r.scene, layer, 0, 0, 1.0).expect("turn");
    r.session.apply(commands).expect("valid");
    let before = where_is(&r, 0, r.parts[1]);
    let commands = detach(r.session.document(), r.scene, layer, 1).expect("detach");
    r.session.apply(commands).expect("valid");
    assert!(close(where_is(&r, 0, r.parts[1]), before));
    let (posed, _) = armature_at(r.session.document(), r.scene, layer, 0).expect("armature");
    assert_eq!(posed.joints()[1].parent(), None);

    // Two keyframes, then back to plain instances.
    let commands = turn_joint(r.session.document(), r.scene, layer, 5, 0, 0.0).expect("turn");
    r.session.apply(commands).expect("valid");
    let commands = release(r.session.document(), r.scene, layer).expect("release");
    r.session.apply(commands).expect("valid");
    let spans = layers(&r)[0].spans();
    assert_eq!(spans.len(), 2);
    assert!(spans.iter().all(|s| s.element_count() == 2));
    assert!(
        spans
            .iter()
            .all(|s| matches!(s.content(), Content::Instance(_)))
    );
    assert!(close(where_is(&r, 0, r.parts[1]), before));
}

#[test]
fn what_cannot_have_a_bone_is_refused_and_changes_nothing() {
    let mut r = rig();
    let doc = r.session.document();
    assert_eq!(
        connect(doc, r.scene, 0, bond(r.parts[0]), bond(r.parts[0]), "A"),
        Err(BoneRefusal::SameInstance)
    );
    let missing = InstanceId::new();
    assert_eq!(
        connect(doc, r.scene, 0, bond(r.parts[0]), bond(missing), "A"),
        Err(BoneRefusal::NoSuchInstance(missing))
    );
    assert_eq!(
        connect(doc, r.scene, 20, bond(r.parts[0]), bond(r.parts[1]), "A"),
        Err(BoneRefusal::NoSuchInstance(r.parts[0])),
        "nothing there at that frame"
    );
    linked(&mut r);
    // Into an armature from outside it.
    assert_eq!(
        connect(
            r.session.document(),
            r.scene,
            0,
            bond(r.parts[2]),
            bond(r.parts[1]),
            "A"
        ),
        Err(BoneRefusal::OtherArmature)
    );
    // A locked layer, and a tweening keyframe.
    let plain = r.plain;
    let tweening = layers(&r)[1].spans()[0].clone().with_tween(Tween::Linear);
    let mut locked = EditorSession::new(r.session.document().clone());
    locked
        .apply([Command::UpdateLayer {
            scene: r.scene,
            layer: plain,
            change: flare_commands::LayerChange::Locked(true),
        }])
        .expect("lock");
    assert_eq!(
        connect(
            locked.document(),
            r.scene,
            0,
            bond(r.parts[1]),
            bond(r.parts[2]),
            "A"
        ),
        Err(BoneRefusal::Locked)
    );
    r.session
        .apply([
            Command::RemoveSpan {
                scene: r.scene,
                layer: plain,
                start: 0,
            },
            Command::InsertSpan {
                scene: r.scene,
                layer: plain,
                span: tweening,
            },
        ])
        .expect("tween");
    assert_eq!(
        connect(
            r.session.document(),
            r.scene,
            0,
            bond(r.parts[1]),
            bond(r.parts[2]),
            "A"
        ),
        Err(BoneRefusal::Tweening)
    );
}

#[test]
fn a_joint_or_a_bone_is_found_under_the_pointer() {
    let mut r = rig();
    let layer = linked(&mut r);
    let at = |x: f64, y: f64| DocumentPoint::new(x, y).expect("finite");
    let doc = r.session.document();
    let near = |x, y| joint_near(doc, r.scene, 0, at(x, y), 2.0);
    let bone = |x, y| bone_near(doc, r.scene, 0, at(x, y), 2.0);
    assert_eq!(near(9.0, 0.5), Some(JointRef { layer, joint: 1 }));
    assert_eq!(near(0.5, 0.0), Some(JointRef { layer, joint: 0 }));
    assert_eq!(near(5.0, 1.0), None, "between the joints");
    // On the bone between them, the tip is what is pulled.
    assert_eq!(bone(5.0, 1.0), Some(JointRef { layer, joint: 1 }));
    assert_eq!(bone(5.0, 5.0), None);
    let skeleton = &skeletons(r.session.document(), r.scene, 0)[0];
    assert_eq!(skeleton.joints[1].1, Some(0));
}
