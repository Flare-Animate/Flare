use super::*;

#[test]
fn function_keys_drive_frame_operations() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws);
    run(&mut ws, &ctx, 0.0);
    ws.set_frame(6);
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::F6, Modifiers::NONE)]),
    );
    assert_eq!(active_spans(&ws), vec![(0, 6), (6, 18)]);
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::F5, Modifiers::NONE)]),
    );
    assert_eq!(active_spans(&ws), vec![(0, 6), (6, 19)]);
    pass(
        &mut ws,
        &ctx,
        input(0.3, vec![key(Key::F5, Modifiers::SHIFT)]),
    );
    assert_eq!(active_spans(&ws), vec![(0, 6), (6, 18)]);
    pass(
        &mut ws,
        &ctx,
        input(0.4, vec![key(Key::F6, Modifiers::SHIFT)]),
    );
    assert_eq!(active_spans(&ws), vec![(0, 24)]);
    ws.set_frame(8);
    pass(
        &mut ws,
        &ctx,
        input(0.5, vec![key(Key::F7, Modifiers::NONE)]),
    );
    assert_eq!(active_spans(&ws), vec![(0, 8), (8, 16)]);
}

#[test]
fn ellipse_and_line_tools_commit_shapes_on_their_own_layers() {
    let mut ws = workspace();
    ws.set_tool(Tool::Ellipse);
    ws.begin_drag(Pos2::new(100.0, 100.0));
    ws.update_drag(Pos2::new(160.0, 140.0));
    ws.end_drag();
    ws.set_tool(Tool::Line);
    ws.begin_drag(Pos2::new(10.0, 10.0));
    ws.update_drag(Pos2::new(90.0, 40.0));
    ws.end_drag();
    let layers = ws.document().scenes()[0].timeline().layers();
    assert_eq!(layers.len(), 3);
    let shape_of = |layer: usize| match layers[layer].spans()[0].content() {
        Content::Shape(shape) => shape.clone(),
        _ => panic!("shape expected"),
    };
    assert_eq!(layers[1].name(), "Ellipse");
    let ellipse = shape_of(1);
    let bounds = ops::shape_bounds(&ellipse).expect("bounds");
    assert_eq!(bounds.min, Pos2::new(100.0, 100.0));
    assert_eq!(bounds.max, Pos2::new(160.0, 140.0));
    assert_eq!(layers[2].name(), "Line");
    let line = shape_of(2);
    assert_eq!(line.path().len(), 2);
    assert!(
        line.stroke().is_some(),
        "a line is its stroke, even with Outline off"
    );
    assert!(!ws.stroke_enabled);
}

#[test]
fn zero_length_lines_and_flat_ellipses_create_nothing() {
    let mut ws = workspace();
    ws.set_tool(Tool::Line);
    ws.begin_drag(Pos2::new(5.0, 5.0));
    ws.end_drag();
    ws.set_tool(Tool::Ellipse);
    ws.begin_drag(Pos2::new(5.0, 5.0));
    ws.update_drag(Pos2::new(50.0, 5.4));
    ws.end_drag();
    assert_eq!(layer_count(&ws), 1);
    assert!(!ws.is_dirty());
}

#[test]
fn motion_tweens_toggle_on_a_selected_instance_and_interpolate() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    assert_eq!(ws.selection_tween(), None, "a shape cannot tween");
    ws.set_selection_tween(Tween::Linear);
    assert!(
        ws.status()
            .is_some_and(|s| s.contains("Select an instance"))
    );
    ws.convert_selection_to_symbol();
    assert_eq!(ws.selection_tween(), Some(Tween::None));
    // Two keyframes with different positions, tweened between them.
    ws.set_frame(10);
    ws.frame_edit(FrameOp::InsertKeyframe);
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.update_drag(Pos2::new(150.0, 30.0)); // moves the second keyframe 100 right
    ws.end_drag();
    ws.set_frame(0);
    ws.select_layer(ws.active_layer_id().expect("layer"));
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.end_drag();
    ws.set_selection_tween(Tween::Linear);
    assert_eq!(ws.selection_tween(), Some(Tween::Linear));
    let scene = ws.scene_id().expect("scene");
    let at_frame = |ws: &Workspace, f: u32| {
        let frame = flare_eval::evaluate_frame(ws.document(), scene, f).expect("eval");
        frame.shapes[0].transform.translation.x
    };
    assert_eq!(at_frame(&ws, 0), 0.0);
    assert_eq!(at_frame(&ws, 5), 50.0, "halfway through the tween");
    assert_eq!(at_frame(&ws, 10), 100.0);
}

#[test]
fn layer_operations_rename_hide_lock_move_and_delete() {
    let mut ws = workspace();
    rect(&mut ws); // layers: [Layer 1 (empty), Rectangle]
    ws.rename_active_layer("  Hero ");
    assert_eq!(active_layer(&ws).name(), "Hero");
    ws.rename_active_layer("   ");
    assert!(ws.status().is_some_and(|s| s.contains("needs a name")));
    assert_eq!(active_layer(&ws).name(), "Hero");

    let id = ws.active_layer_id().expect("id");
    ws.toggle_layer_visible(id);
    assert!(!active_layer(&ws).is_visible());
    ws.toggle_layer_visible(id);
    assert!(active_layer(&ws).is_visible());

    ws.move_active_layer(-1);
    assert_eq!(ws.document().scenes()[0].timeline().layers()[0].id(), id);
    ws.move_active_layer(-1);
    assert!(ws.status().is_some_and(|s| s.contains("edge")));
    ws.delete_active_layer();
    assert_eq!(layer_count(&ws), 1);
    ws.delete_active_layer();
    assert!(
        ws.status()
            .is_some_and(|s| s.contains("at least one layer"))
    );
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn locked_layers_are_not_selectable_and_refuse_frame_edits() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    let id = ws.active_layer_id().expect("id");
    ws.toggle_layer_locked(id);
    assert!(
        !ws.has_selection(),
        "locking drops a selection on that layer"
    );
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(50.0, 30.0));
    assert!(!ws.has_selection(), "clicks go through a locked layer");
    ws.frame_edit(FrameOp::InsertFrame);
    assert!(ws.status().is_some_and(|s| s.contains("locked")));
    ws.toggle_layer_locked(id);
    ws.begin_drag(Pos2::new(50.0, 30.0));
    assert!(ws.has_selection());
}

#[test]
fn clicks_select_through_a_locked_layer_to_the_one_below() {
    let mut ws = workspace();
    ws.stroke_enabled = false;
    ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(100.0, 100.0)); // lower layer
    ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(100.0, 100.0)); // new layer, on top? (below in order)
    let layers: Vec<LayerId> = ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(Layer::id)
        .collect();
    // Layers are top-to-bottom; the last one is drawn first, so layers[1] paints over layers[2].
    ws.toggle_layer_locked(layers[1]);
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(50.0, 50.0));
    assert_eq!(ws.selected.map(|s| s.layer), Some(layers[2]));
}

#[test]
fn hidden_layers_do_not_render_or_hit() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    let id = ws.active_layer_id().expect("id");
    ws.toggle_layer_visible(id);
    assert!(ws.hit(Pos2::new(50.0, 30.0)).is_none());
    let scene = ws.scene_id().expect("scene");
    let frame = flare_eval::evaluate_frame(ws.document(), scene, 0).expect("eval");
    assert!(frame.shapes.is_empty());
}

#[test]
fn editing_a_symbol_in_place_applies_to_the_parent_as_one_undo_step() {
    let (mut ws, symbol) = with_symbol();
    let before = ws.document().clone();
    ws.edit_selected_symbol();
    assert!(ws.in_symbol());
    assert_eq!(
        ws.breadcrumb(),
        vec!["Scene 1".to_owned(), "Symbol 1".to_owned()]
    );
    assert_eq!(ws.document().scenes().len(), 1);
    assert_eq!(ws.frame(), 0);

    ws.set_tool(Tool::Rectangle);
    ws.commit_rectangle(Pos2::new(300.0, 300.0), Pos2::new(340.0, 330.0));
    assert!(ws.is_dirty());
    assert_eq!(
        symbol_layers(&ws, symbol),
        1,
        "the parent is untouched until you leave"
    );

    assert!(ws.exit_symbol());
    assert!(!ws.in_symbol());
    assert_eq!(symbol_layers(&ws, symbol), 2);
    ws.undo();
    assert_eq!(ws.document(), &before, "leaving was a single undo step");
}

#[test]
fn undo_inside_a_symbol_is_local_to_that_symbol() {
    let (mut ws, symbol) = with_symbol();
    ws.edit_selected_symbol();
    ws.commit_rectangle(Pos2::new(300.0, 300.0), Pos2::new(340.0, 330.0));
    assert_eq!(layer_count(&ws), 2);
    ws.undo();
    assert_eq!(layer_count(&ws), 1);
    ws.undo(); // nothing left to undo inside; the scene's own history is not touched
    assert_eq!(layer_count(&ws), 1);
    assert!(ws.exit_symbol());
    assert_eq!(
        symbol_layers(&ws, symbol),
        1,
        "no net change leaves no edit behind"
    );
    assert_eq!(ws.document().symbols().len(), 1);
}

#[test]
fn symbols_created_inside_a_symbol_join_the_library_when_you_leave() {
    let (mut ws, outer) = with_symbol();
    ws.edit_selected_symbol();
    // Inside: select the shape and convert it, creating a nested symbol.
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.end_drag();
    assert!(ws.can_convert_selection());
    ws.convert_selection_to_symbol();
    let inner = ws.selected_symbol.expect("inner symbol");
    assert_ne!(inner, outer);
    assert!(ws.exit_symbol());
    assert!(
        ws.document().symbol(inner).is_some(),
        "the new symbol reached the library"
    );
    assert_eq!(ws.document().symbols().len(), 2);
    // The outer symbol now contains an instance of the new one.
    let outer_symbol = ws.document().symbol(outer).expect("outer");
    assert!(
        outer_symbol
            .timeline()
            .referenced_symbols()
            .any(|s| s == inner)
    );
}

#[test]
fn nested_symbols_can_be_entered_and_left_by_breadcrumb_depth() {
    let (mut ws, _) = with_symbol();
    ws.edit_selected_symbol();
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.end_drag();
    ws.convert_selection_to_symbol();
    ws.edit_selected_symbol();
    assert_eq!(ws.breadcrumb().len(), 3);
    assert!(ws.exit_to_depth(0));
    assert!(!ws.in_symbol());
    assert_eq!(ws.document().symbols().len(), 2);
}

#[test]
fn a_symbol_cannot_contain_itself_or_its_own_ancestors() {
    let (mut ws, outer) = with_symbol();
    ws.edit_selected_symbol();
    ws.selected_symbol = Some(outer);
    ws.place_selected_symbol();
    assert!(
        ws.status()
            .is_some_and(|s| s.contains("cannot contain itself"))
    );
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn a_rejected_exit_keeps_the_edit_open_with_the_reason() {
    use flare_document::{Instance, Symbol};
    // B contains A. Editing A and placing B inside it would make a loop.
    let a = SymbolId::new();
    let b = SymbolId::new();
    let instance = |symbol| {
        let i = Instance::new(
            flare_core::InstanceId::new(),
            symbol,
            flare_core::Transform2D::IDENTITY,
        );
        Content::Instance(i)
    };
    let one = |content| {
        let span = FrameSpan::new(0, NonZeroU32::MIN, content);
        Timeline::new(vec![
            Layer::new(LayerId::new(), "L", vec![span]).expect("layer"),
        ])
    };
    let base = document();
    let doc = Document::new(
        base.id(),
        base.frame_rate(),
        base.scenes().to_vec(),
        vec![
            Symbol::new(a, "A", Timeline::default()),
            Symbol::new(b, "B", one(instance(a))),
        ],
    )
    .expect("doc");
    let mut ws = Workspace::new(doc, None);
    assert!(ws.enter_symbol(a));
    ws.selected_symbol = Some(b);
    ws.place_selected_symbol(); // allowed inside the scratch: B is not A
    assert_eq!(layer_count(&ws), 1, "a layer with B was added");
    assert!(!ws.exit_symbol(), "A containing B containing A is a loop");
    assert!(ws.in_symbol(), "still editing A");
    assert!(ws.status().is_some_and(|s| s.contains("Cannot leave")));
    ws.undo();
    assert!(ws.exit_symbol(), "after undoing the loop, leaving works");
}

#[test]
fn saving_inside_a_symbol_applies_pending_edits_first() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let (mut ws, symbol) = with_symbol();
    ws.edit_selected_symbol();
    ws.commit_rectangle(Pos2::new(300.0, 300.0), Pos2::new(340.0, 330.0));
    ws.save_as(&path);
    assert!(!ws.in_symbol(), "saving left the symbol");
    assert!(!ws.is_dirty());
    let mut reopened = workspace();
    reopened.open(&path);
    assert_eq!(symbol_layers(&reopened, symbol), 2);
}

#[test]
fn entering_and_leaving_refresh_the_stage_and_missing_symbols_are_reported() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    let before = ws.texture.as_ref().expect("texture").key;
    assert!(!ws.enter_symbol(SymbolId::new()));
    assert!(ws.status().is_some_and(|s| s.contains("no longer exists")));

    let (mut ws, _) = with_symbol();
    run(&mut ws, &ctx, 0.1);
    let scene_key = ws.texture.as_ref().expect("texture").key;
    ws.edit_selected_symbol();
    run(&mut ws, &ctx, 0.2);
    let symbol_key = ws.texture.as_ref().expect("texture").key;
    assert_ne!(
        scene_key.0, symbol_key.0,
        "contexts never share a cache key"
    );
    assert!(before.0 == 0);
}

#[test]
fn folders_wrap_the_active_layer_hide_and_lock_what_they_hold_and_let_go_when_deleted() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.perform(MenuAction::SelectAll);
    let art = ws.active_layer_id().expect("the drawing's layer is active");
    ws.perform(MenuAction::NewLayerFolder);
    let layers_of = |ws: &Workspace| ws.document().scenes()[0].timeline().layers().to_vec();
    let before = layers_of(&ws);
    let folder = before
        .iter()
        .find(|l| l.is_folder())
        .expect("a folder")
        .id();
    let held = before.iter().find(|l| l.id() == art).expect("the layer");
    assert_eq!(held.parent(), Some(folder), "the active layer went into it");
    let at = |id| before.iter().position(|l| l.id() == id).expect("position");
    assert!(at(folder) < at(art), "a folder sits above what it holds");
    assert_eq!(ws.active_layer_id(), Some(folder));

    // Locking the folder makes its layers unselectable; hiding it removes them from the picture.
    ws.toggle_layer_locked(folder);
    ws.select_at(Pos2::new(50.0, 30.0), false);
    assert!(ws.selected.is_none(), "a locked folder locks what is in it");
    ws.toggle_layer_locked(folder);
    ws.select_at(Pos2::new(50.0, 30.0), false);
    assert!(ws.selected.is_some());
    ws.toggle_layer_visible(folder);
    ws.clear_selection();
    ws.select_at(Pos2::new(50.0, 30.0), false);
    assert!(ws.selected.is_none(), "a hidden folder hides what is in it");
    ws.toggle_layer_visible(folder);

    // Layers can be moved out of a folder and into one that is above them.
    ws.select_layer(art);
    assert!(
        ws.folders_for_active_layer().is_empty(),
        "its own folder is not offered again until it leaves"
    );
    ws.move_active_layer_to_folder(None);
    assert_eq!(ws.layer(art).and_then(|l| l.parent()), None);
    assert_eq!(ws.folders_for_active_layer().len(), 1);
    ws.move_active_layer_to_folder(Some(folder));
    assert_eq!(ws.layer(art).and_then(|l| l.parent()), Some(folder));

    // Deleting the folder lets its layers out instead of refusing or taking them along.
    ws.select_layer(folder);
    ws.delete_active_layer();
    assert!(layers_of(&ws).iter().all(|l| !l.is_folder()));
    assert_eq!(ws.layer(art).and_then(|l| l.parent()), None);
    ws.undo();
    assert!(layers_of(&ws).iter().any(|l| l.is_folder()));
}
