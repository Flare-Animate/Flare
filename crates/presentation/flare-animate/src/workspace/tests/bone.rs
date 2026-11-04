//! The Bone tool links instances on the stage, and the Selection tool poses what it made.

use std::num::NonZeroU32;

use flare_core::{InstanceId, Rgba8, ShapeId, Transform2D};
use flare_document::{FrameSpan, Instance, Shape, Symbol};
use glamx::{DAffine2, DVec2};

use super::*;

/// Two instances of a 20 px square centered on (100, 100) and (200, 100), in one keyframe of 10
/// frames.
fn two_parts() -> (Workspace, [InstanceId; 2]) {
    let square = Shape::rect(
        ShapeId::new(),
        -10.0,
        -10.0,
        20.0,
        20.0,
        Rgba8::opaque(255, 0, 0),
    )
    .expect("rect");
    let art = Layer::new(
        LayerId::new(),
        "L",
        vec![FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(square))],
    )
    .expect("layer");
    let symbol = Symbol::new(SymbolId::new(), "Part", Timeline::new(vec![art]));
    let part = |x: f64| {
        let at = DAffine2::from_translation(DVec2::new(x, 100.0));
        Instance::new(
            InstanceId::new(),
            symbol.id(),
            Transform2D::new(at).expect("finite"),
        )
    };
    let (a, b) = (part(100.0), part(200.0));
    let ids = [a.id(), b.id()];
    let span = FrameSpan::new(0, NonZeroU32::new(10).expect("10"), Content::Instance(a))
        .with_extras(vec![Content::Instance(b)])
        .expect("elements");
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![span]).expect("layer");
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![Scene::new(
            SceneId::new(),
            "Scene 1",
            Timeline::new(vec![layer]),
        )],
        vec![symbol],
    )
    .expect("document");
    (Workspace::new(document, None), ids)
}

fn armature(ws: &Workspace) -> Option<flare_document::Armature> {
    ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find_map(|l| match l.span_at(ws.frame)?.content() {
            Content::Armature(a) => Some(a.clone()),
            _ => None,
        })
}

fn drag(ws: &mut Workspace, from: (f32, f32), to: (f32, f32)) -> bool {
    let started = ws.bone_press(Pos2::new(from.0, from.1));
    ws.bone_move(Pos2::new((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0));
    ws.bone_release(Pos2::new(to.0, to.1));
    started
}

#[test]
fn the_bone_tool_is_built_and_a_drag_between_two_instances_links_them_in_one_step() {
    let (mut ws, [first, second]) = two_parts();
    ws.set_tool(Tool::Bone);
    assert_eq!(ws.tool_hint(), ws.tr("tools-status-bone"));
    assert!(drag(&mut ws, (102.0, 98.0), (197.0, 103.0)));
    let rig = armature(&ws).expect("an armature");
    assert_eq!(rig.joints().len(), 2);
    assert_eq!(rig.joints()[0].instance().id(), first);
    assert_eq!(rig.joints()[1].instance().id(), second);
    // The joints go in the middle of each instance, wherever the drag began.
    assert_eq!(rig.joints()[0].at().as_vec(), DVec2::new(100.0, 100.0));
    assert_eq!(rig.joints()[1].at().as_vec(), DVec2::new(200.0, 100.0));
    assert_eq!(layer_count(&ws), 2);
    assert_eq!(layer_names(&ws)[0], "Armature_1");
    ws.undo();
    assert!(armature(&ws).is_none());
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn letting_go_over_nothing_makes_no_bone_and_says_why() {
    let (mut ws, _) = two_parts();
    ws.set_tool(Tool::Bone);
    drag(&mut ws, (100.0, 100.0), (400.0, 300.0));
    assert!(armature(&ws).is_none());
    assert_eq!(
        ws.status.as_deref(),
        Some(ws.tr("tools-bone-drop-on-instance").as_str())
    );
    // Neither is a press on empty stage the start of a bone.
    drag(&mut ws, (400.0, 300.0), (100.0, 100.0));
    assert!(armature(&ws).is_none());
}

#[test]
fn the_selection_tool_poses_the_armature_by_its_joints() {
    let (mut ws, _) = two_parts();
    ws.set_tool(Tool::Bone);
    drag(&mut ws, (100.0, 100.0), (200.0, 100.0));
    ws.set_tool(Tool::Select);
    // A press away from the armature is the Selection tool's own.
    assert!(!ws.bone_press(Pos2::new(400.0, 300.0)));
    ws.set_frame(5);
    // Pull the second joint down: the bone turns a quarter turn about the first.
    assert!(drag(&mut ws, (200.0, 100.0), (100.0, 200.0)));
    let posed = armature(&ws).expect("armature").posed();
    assert!(
        posed[1].0.distance(DVec2::new(100.0, 200.0)) < 0.01,
        "{posed:?}"
    );
    // The pose is a keyframe at frame 5; frame 0 kept its own.
    ws.set_frame(0);
    let rest = armature(&ws).expect("armature").posed();
    assert!(rest[1].0.distance(DVec2::new(200.0, 100.0)) < 1e-9);
    // Properties edits the joint that was touched.
    ws.set_frame(5);
    let (joint, _) = ws.bone_joint_here().expect("a joint");
    assert_eq!(joint.joint, 1);
    let one_step = ws.active().undo_labels().count();
    ws.undo();
    assert_eq!(ws.active().undo_labels().count(), one_step - 1);
}

#[test]
fn a_press_on_an_instance_the_armature_carries_poses_it_and_the_root_moves_it_all() {
    let (mut ws, _) = two_parts();
    ws.set_tool(Tool::Bone);
    drag(&mut ws, (100.0, 100.0), (200.0, 100.0));
    ws.set_tool(Tool::Select);
    // Grab the first instance near its edge, off the joint itself: it is the root.
    assert!(drag(&mut ws, (108.0, 94.0), (128.0, 114.0)));
    let posed = armature(&ws).expect("armature").posed();
    assert!(
        posed[0].0.distance(DVec2::new(120.0, 120.0)) < 1e-6,
        "{posed:?}"
    );
    assert!(posed[1].0.distance(DVec2::new(220.0, 120.0)) < 1e-6);
}

/// Where stage point `at` is on the screen, read from where the workspace says the pointer is.
fn on_screen(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, at: Pos2) -> Pos2 {
    let (a, b) = (Pos2::new(500.0, 350.0), Pos2::new(700.0, 450.0));
    let mut stage_of = |p: Pos2| {
        *time += 0.02;
        pass(ws, ctx, input(*time, vec![Event::PointerMoved(p)]));
        ws.pointer_stage.expect("over the stage")
    };
    let (sa, sb) = (stage_of(a), stage_of(b));
    let scale = (b.x - a.x) / (sb.x - sa.x);
    Pos2::new(a.x + (at.x - sa.x) * scale, a.y + (at.y - sa.y) * scale)
}

#[test]
fn a_drag_on_the_stage_with_the_bone_tool_makes_the_bone_and_draws_it() {
    let (mut ws, _) = two_parts();
    ws.set_tool(Tool::Bone);
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let mut time = 0.1;
    let from = on_screen(&mut ws, &ctx, &mut time, Pos2::new(100.0, 100.0));
    let to = on_screen(&mut ws, &ctx, &mut time, Pos2::new(200.0, 100.0));
    let button = |at, pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass(
        &mut ws,
        &ctx,
        input(time + 0.02, vec![Event::PointerMoved(from)]),
    );
    time += 0.04;
    pass(&mut ws, &ctx, input(time, vec![button(from, true)]));
    for step in 1..=6 {
        time += 0.02;
        let at = from + (to - from) * (step as f32 / 6.0);
        pass(&mut ws, &ctx, input(time, vec![Event::PointerMoved(at)]));
    }
    assert!(ws.bone_drag.is_some(), "the drag is a bone being drawn");
    pass(&mut ws, &ctx, input(time + 0.02, vec![button(to, false)]));
    assert!(ws.bone_drag.is_none());
    assert_eq!(armature(&ws).expect("an armature").joints().len(), 2);
    // The bone is drawn over the stage: a wedge in the bone's color.
    let mut output = ctx.run_ui(input(time + 0.1, vec![]), |ui| ws.show(ui));
    output.textures_delta.clear();
    let bone = Color32::from_rgb(200, 220, 120);
    assert!(
        output.shapes.iter().any(|s| matches!(&s.shape,
            egui::epaint::Shape::Path(p) if p.stroke.color == egui::epaint::ColorMode::Solid(bone))),
        "the bone is on the stage"
    );
}
