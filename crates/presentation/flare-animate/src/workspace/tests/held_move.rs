//! Moving what a layer only holds on a frame starts a keyframe there; the frames before keep
//! what they showed.

use super::*;

fn held_rectangle() -> (Workspace, LayerId) {
    let mut ws = wide_rectangle_selected();
    let layer = ws.current_layer.expect("layer");
    let scene = ws.scene_id().expect("scene");
    ws.active_mut()
        .apply([Command::UpdateLayer {
            scene,
            layer,
            change: flare_commands::LayerChange::Hold(true),
        }])
        .expect("hold");
    (ws, layer)
}

fn bounds_at(ws: &Workspace, layer: LayerId, start: u32) -> Rect {
    let scene = ws.scene_id().expect("scene");
    let at = SpanRef {
        layer,
        start,
        element: 0,
    };
    ops::span_bounds(ws.document(), scene, at).expect("bounds")
}

fn starts(ws: &Workspace, layer: LayerId) -> Vec<u32> {
    ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .find(|l| l.id() == layer)
        .expect("layer")
        .spans()
        .iter()
        .map(FrameSpan::start)
        .collect()
}

#[test]
fn nudging_a_held_drawing_starts_a_keyframe_and_keeps_the_frames_before() {
    let (mut ws, layer) = held_rectangle();
    // Past the span's end the layer only holds the drawing.
    let end = ws.document().scenes()[0].timeline().layers()[0].spans()[0]
        .end()
        .try_into()
        .expect("fits");
    let before = bounds_at(&ws, layer, 0);
    ws.set_frame_in_view(end, end + 1);
    ws.select_at(Pos2::new(60.0, 35.0), false);
    assert_eq!(ws.selected.map(|s| s.start), Some(0), "the held keyframe");
    ws.move_selection(egui::Vec2::new(20.0, 0.0));
    assert_eq!(starts(&ws, layer), vec![0, end], "a keyframe starts here");
    assert_eq!(
        bounds_at(&ws, layer, 0),
        before,
        "frames before are as they were"
    );
    assert!((bounds_at(&ws, layer, end).min.x - before.min.x - 20.0).abs() < 1e-3);
    assert_eq!(
        ws.selected.map(|s| s.start),
        Some(end),
        "the selection follows"
    );
    ws.undo();
    assert_eq!(starts(&ws, layer), vec![0], "one undo takes it all back");
    assert_eq!(bounds_at(&ws, layer, 0), before);
}
