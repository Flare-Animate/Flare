//! What the Properties panel offers for a selected tween or symbol instance, and where the
//! selection box sits while the playhead is inside a tween.

use super::*;
use flare_document::SymbolKind;

/// An instance on frames 0 to 9 and another at frame 10, 100 units to the right, joined by a
/// motion tween. The instance at frame 0 is selected, the playhead on `frame`.
fn tweened_instance(frame: u32) -> Workspace {
    let mut ws = workspace();
    ws.library.new_kind = SymbolKind::MovieClip;
    draw_and_select(&mut ws);
    ws.convert_selection_to_symbol();
    ws.set_frame(10);
    ws.frame_edit(FrameOp::InsertKeyframe);
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.update_drag(Pos2::new(150.0, 30.0));
    ws.end_drag();
    ws.set_frame(0);
    ws.select_layer(ws.active_layer_id().expect("layer"));
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.end_drag();
    ws.perform(MenuAction::ToggleMotionTween);
    assert!(
        ws.selection_tween()
            .is_some_and(|t| t != flare_document::Tween::None)
    );
    ws.set_frame(frame);
    ws
}

fn drawn(ws: &mut Workspace) -> Vec<String> {
    let ctx = egui::Context::default();
    flare_ui::icons::install(&ctx);
    let mut all = Vec::new();
    for step in 0..4 {
        let mut out = ctx.run_ui(input(f64::from(step) * 0.1, vec![]), |ui| ws.show(ui));
        out.textures_delta.clear();
        all = out
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::Text(t) => Some(t.galley.text().to_owned()),
                _ => None,
            })
            .collect();
    }
    all
}

#[test]
fn the_selection_box_follows_the_instance_mid_tween() {
    let ws = tweened_instance(0);
    let start = ws.selection_bounds().expect("box").min.x;
    let mut ws = tweened_instance(5);
    let mid = ws.selection_bounds().expect("box").min.x;
    assert!(
        (mid - (start + 50.0)).abs() < 1.0,
        "start {start}, mid {mid}"
    );
    ws.set_frame(0);
    assert!((ws.selection_bounds().expect("box").min.x - start).abs() < 1e-3);
}

#[test]
fn a_selected_instance_in_a_tween_offers_the_curve_the_type_and_the_edit_button() {
    let mut ws = tweened_instance(0);
    let text = drawn(&mut ws);
    for wanted in ["Tween", "Curve", "Ease slider", "Type", "Movie Clip"] {
        assert!(text.iter().any(|t| t == wanted), "{wanted}: {text:?}");
    }
}

#[test]
fn choosing_an_interpolation_from_the_object_is_one_undo_step() {
    let mut ws = tweened_instance(0);
    let eased = flare_tools::tween::spec::families()
        .into_iter()
        .find(|f| f.eased)
        .expect("an eased family");
    let easing = flare_tools::tween::spec::easing_names()
        .into_iter()
        .next()
        .expect("an easing");
    let interp = flare_tools::tween::spec::build_interp(eased.name, Some(easing), None, None);
    assert!(interp.is_ok(), "{interp:?}");
    let interp = interp.ok();
    ws.set_selection_interp(interp);
    assert!(ws.selection_interp().is_some());
    let text = drawn(&mut ws);
    assert!(text.iter().any(|t| t == "Easing"), "{text:?}");
    ws.undo();
    assert!(ws.selection_interp().is_none());
}

#[test]
fn the_type_of_the_selected_instance_is_changed_from_its_symbol_as_one_undo_step() {
    let mut ws = tweened_instance(0);
    let scene = ws.scene_id().expect("scene");
    let at = ws.selected.expect("selected");
    let Some(Content::Instance(instance)) = ops::find_element(ws.document(), scene, at) else {
        panic!("an instance");
    };
    let id = instance.symbol();
    ws.set_symbol_kind(id, SymbolKind::Graphic);
    assert_eq!(
        ws.document().symbol(id).map(|s| s.kind()),
        Some(SymbolKind::Graphic)
    );
    // A Graphic has its looping choice (Loop, Play Once, Single Frame) and first frame.
    let text = drawn(&mut ws);
    for wanted in ["Looping", "First", "Loop"] {
        assert!(text.iter().any(|t| t == wanted), "{wanted}: {text:?}");
    }
    ws.undo();
    assert_eq!(
        ws.document().symbol(id).map(|s| s.kind()),
        Some(SymbolKind::MovieClip)
    );
}
