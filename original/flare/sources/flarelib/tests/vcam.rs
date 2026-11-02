//! Free camera (VCam): follow, shake, colour effects, mode.

use ac_core::commands::execute;
use ac_core::*;
use serde_json::json;

fn scene() -> Editor {
    let mut ed = Editor::new(Document::default()); // 550 × 400
    execute(&mut ed, "draw.rect", &json!({"x": 0, "y": 0, "w": 20, "h": 20, "fill": "#FF0000"})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.to_symbol", &json!({"name": "Hero", "kind": "graphic"})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.set", &json!({"name": "hero"})).unwrap();
    // hero walks from x 0 to 400 over 11 frames
    execute(&mut ed, "timeline.insert_keyframe", &json!({"frame": 11})).unwrap();
    execute(&mut ed, "select.all", &json!({})).unwrap();
    execute(&mut ed, "selection.move", &json!({"dx": 400, "dy": 0})).unwrap();
    execute(&mut ed, "timeline.set_tween", &json!({"kind": "classic", "frame": 1})).unwrap();
    ed
}

#[test]
fn camera_mode_round_trips_and_defaults_to_stage() {
    let mut ed = scene();
    assert_eq!(ed.doc.camera_mode, CameraMode::Stage);
    execute(&mut ed, "camera.mode", &json!({"mode": "free"})).unwrap();
    assert_eq!(ed.doc.camera_mode, CameraMode::Free);
    let back = Document::from_json(&ed.doc.to_json()).unwrap();
    assert_eq!(back.camera_mode, CameraMode::Free);
    assert!(execute(&mut ed, "camera.mode", &json!({"mode": "orbit"})).is_err());
}

#[test]
fn camera_follows_a_named_object_with_lag_and_offset() {
    let mut ed = scene();
    execute(&mut ed, "camera.options", &json!({"follow": "hero", "frame": 1})).unwrap();
    let c = |ed: &Editor, f: u32| camera_element(&ed.doc, ed.scene_tl(), f).unwrap().matrix;
    // hero centre at frame 6 (0-based 5): x = 10 + 200
    let m = c(&ed, 5);
    assert!((m.tx - 210.0).abs() < 1.0 && (m.ty - 10.0).abs() < 1.0, "{m:?}");
    // the view puts the hero at the stage centre
    let v = camera_view(&ed.doc, ed.scene_tl(), 5).unwrap();
    let p = v.apply(pt(210.0, 10.0));
    assert!((p.x - 275.0).abs() < 1.0 && (p.y - 200.0).abs() < 1.0);
    // lag: the camera trails behind; offset: shifts the framing
    execute(&mut ed, "camera.options", &json!({"follow_lag": 4, "offset_x": 50, "frame": 1})).unwrap();
    let m = c(&ed, 5);
    assert!((m.tx - (130.0 + 50.0)).abs() < 1.0, "average of frames 2-6 (50..210) plus 50: {}", m.tx);
}

#[test]
fn camera_shake_is_deterministic_and_bounded() {
    let mut ed = scene();
    execute(&mut ed, "camera.options", &json!({"shake": 6, "shake_speed": 10, "frame": 1})).unwrap();
    let pos = |f| camera_element(&ed.doc, ed.scene_tl(), f).unwrap().matrix;
    let base = ed.camera_at(3).unwrap();
    let mut moved = false;
    for f in 0..11 {
        let (a, b) = (pos(f), pos(f));
        assert_eq!(a, b, "same frame, same shake");
        let (dx, dy) = (a.tx - base.tx, a.ty - base.ty);
        assert!(dx.abs() <= 6.01 && dy.abs() <= 6.01);
        moved |= dx.abs() > 0.5 || dy.abs() > 0.5;
    }
    assert!(moved);
    // turning it off removes the options
    execute(&mut ed, "camera.options", &json!({"shake": 0, "frame": 1})).unwrap();
    let cam = ed.camera_layer().unwrap();
    assert!(ed.scene_tl().layers[cam].keyframes[&0].camera.is_none());
}

#[test]
fn camera_effects_apply_to_the_whole_view() {
    let mut ed = scene();
    execute(&mut ed, "camera.effect", &json!({"brightness": -50, "alpha": 80, "blur": 4, "frame": 1})).unwrap();
    let items = evaluate_scene(&ed.doc, 0);
    assert!(!items.is_empty());
    for it in &items {
        assert!((it.alpha - 0.8).abs() < 1e-3);
        assert_eq!(it.tint, Some((Color::rgb(0, 0, 0), 0.5)));
        assert!(matches!(it.fx.first().map(|g| &g.filters[0]), Some(Filter::Blur { .. })));
    }
}

#[test]
fn a_camera_keyframe_made_mid_span_keeps_follow_and_shake() {
    let mut ed = scene();
    execute(&mut ed, "camera.options", &json!({"follow": "hero", "shake": 4, "frame": 1})).unwrap();
    // a colour effect at frame 6 splits the camera span
    execute(&mut ed, "camera.effect", &json!({"brightness": 0.3, "frame": 6})).unwrap();
    ed.set_frame(5);
    let o = ed.camera_opts().unwrap();
    assert_eq!(o.follow.as_deref(), Some("hero"));
    assert_eq!(o.shake, 4.0);
}
