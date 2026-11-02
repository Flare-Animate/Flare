use flare_core::DocumentPoint;
use flare_document::Camera;
use flare_eval::camera_view;

use super::*;

fn camera_layer_of(ws: &Workspace) -> Option<flare_document::Layer> {
    ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.is_camera())
        .cloned()
}

fn camera_at_frame(ws: &Workspace, frame: u32) -> Camera {
    flare_editor::camera_now(ws.document(), ws.scene_id().expect("scene"), frame).expect("camera")
}

fn aimed(ws: &mut Workspace, x: f64, y: f64, zoom: f64) {
    let camera = Camera::new(DocumentPoint::new(x, y).expect("finite"), zoom, 0.0).expect("camera");
    ws.key_camera(ws.frame, camera, None, "Test camera");
}

#[test]
fn picking_the_camera_tool_adds_a_camera_layer_once_and_undo_takes_it_away() {
    let mut ws = workspace();
    rect(&mut ws);
    let layers = layer_count(&ws);
    ws.set_tool(Tool::Camera);
    assert_eq!(layer_count(&ws), layers + 1);
    let camera = camera_layer_of(&ws).expect("a camera layer");
    assert_eq!(
        ws.document().scenes()[0].timeline().layers()[0].id(),
        camera.id(),
        "on top"
    );
    // It looks at the stage as it is.
    let stage = ws.document().stage();
    assert_eq!(camera_at_frame(&ws, 0), Camera::centered(&stage));
    // Picking another tool and coming back does not add a second one.
    ws.set_tool(Tool::Select);
    ws.set_tool(Tool::Camera);
    assert_eq!(layer_count(&ws), layers + 1);
    // One step undoes the whole of it.
    ws.undo();
    assert!(camera_layer_of(&ws).is_none());
    assert_eq!(layer_count(&ws), layers);
}

#[test]
fn dragging_with_the_camera_tool_moves_the_camera_the_other_way_at_the_current_frame() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.set_tool(Tool::Camera);
    aimed(&mut ws, 100.0, 50.0, 2.0);
    // The picture follows the pointer: 40 units right at zoom 2 is 20 units of the scene, so the
    // camera looks 20 further left.
    ws.pan_camera(egui::Vec2::new(40.0, 0.0));
    let after = camera_at_frame(&ws, 0);
    assert!(
        (after.center().as_vec().x - 80.0).abs() < 1e-9,
        "{:?}",
        after.center()
    );
    assert!((after.center().as_vec().y - 50.0).abs() < 1e-9);
    assert_eq!(after.zoom(), 2.0);
    // At a later frame it keys there, leaving the first frame alone.
    ws.set_frame(10);
    ws.pan_camera(egui::Vec2::new(0.0, 20.0));
    assert!((camera_at_frame(&ws, 0).center().as_vec().x - 80.0).abs() < 1e-9);
    assert!((camera_at_frame(&ws, 10).center().as_vec().y - 40.0).abs() < 1e-9);
}

#[test]
fn what_is_picked_stays_in_the_scenes_own_coordinates_whatever_the_camera_does() {
    let mut ws = workspace();
    rect(&mut ws); // covers 10..110 x 10..60
    ws.set_tool(Tool::Camera);
    aimed(&mut ws, 500.0, 500.0, 4.0);
    assert!(
        ws.hit(Pos2::new(50.0, 30.0)).is_some(),
        "the shape is where it was drawn"
    );
    assert!(ws.hit(Pos2::new(900.0, 900.0)).is_none());
    // The picture, though, is moved: the camera's view is not the identity.
    let timeline = ws.document().scenes()[0].timeline();
    assert_ne!(
        camera_view(ws.document(), timeline, 0),
        glamx::DAffine2::IDENTITY
    );
}

#[test]
fn previewing_through_the_camera_redraws_and_goes_back_when_the_camera_goes() {
    let mut ws = workspace();
    rect(&mut ws);
    assert!(!ws.camera_preview);
    ws.set_tool(Tool::Camera);
    ws.set_camera_preview(true);
    assert!(ws.camera_preview);
    assert!(ws.texture.is_none(), "the picture is drawn again");
    // Remove the camera by undo; the preview cannot outlive it.
    ws.undo();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    assert!(!ws.camera_preview);
}

#[test]
fn the_cameras_keyframes_take_a_motion_tween_from_the_timeline() {
    use flare_tools::ops::{self, SpanRef};
    let mut ws = workspace();
    rect(&mut ws);
    ws.set_tool(Tool::Camera);
    aimed(&mut ws, 100.0, 50.0, 1.0);
    ws.set_frame(12);
    aimed(&mut ws, 150.0, 50.0, 2.0);
    let layer = camera_layer_of(&ws).expect("camera");
    let scene = ws.scene_id().expect("scene");
    let first = SpanRef {
        layer: layer.id(),
        start: 0,
        element: 0,
    };
    let commands = ops::set_tween(ws.document(), scene, first, flare_document::Tween::Linear)
        .expect("a camera keyframe tweens");
    ws.active_mut()
        .apply_labeled(commands, "Tween")
        .expect("applies");
    let halfway = camera_at_frame(&ws, 6);
    assert!(
        halfway.zoom() > 1.0 && halfway.zoom() < 2.0,
        "{}",
        halfway.zoom()
    );
}
