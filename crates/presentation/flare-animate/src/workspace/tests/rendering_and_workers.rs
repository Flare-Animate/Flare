use super::*;

#[test]
fn frames_that_look_the_same_are_rendered_once() {
    let (ctx, mut ws) = (egui::Context::default(), held_then_changing());
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.01); // the panel is known now, so the view settles
    run(&mut ws, &ctx, 0.02);
    let settled = ws.frames_rendered;
    // Frames 1..12 show exactly the picture of frame 0: no further rendering.
    for frame in 1..12 {
        ws.set_frame(frame);
        run(&mut ws, &ctx, 0.1 + f64::from(frame) * 0.01);
    }
    assert_eq!(
        ws.frames_rendered, settled,
        "a held exposure is rendered once"
    );
    assert_eq!(
        ws.texture.as_ref().map(|t| t.key.2),
        Some(11),
        "but the right frame is current"
    );
}

#[test]
fn a_looping_animation_costs_nothing_after_the_first_pass() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(40.0, 40.0));
    ws.set_tool(Tool::Select);
    let layer = ws.current_layer.expect("layer");
    for frame in [8, 16] {
        ws.set_frame(frame);
        ws.current_layer = Some(layer);
        ws.frame_edit(FrameOp::InsertKeyframe);
    }
    ws.set_frame(0);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.01);
    let first = ws.frames_rendered;
    for frame in 0..24 {
        ws.set_frame(frame);
        run(&mut ws, &ctx, 0.2 + f64::from(frame) * 0.01);
    }
    let after_one_pass = ws.frames_rendered;
    for frame in 0..24 {
        ws.set_frame(frame);
        run(&mut ws, &ctx, 1.0 + f64::from(frame) * 0.01);
    }
    assert_eq!(
        ws.frames_rendered, after_one_pass,
        "the second loop renders nothing"
    );
    assert!(after_one_pass >= first);
}

#[test]
fn editing_invalidates_cached_pictures_and_the_cache_stays_within_its_limits() {
    let (ctx, mut ws) = (egui::Context::default(), held_then_changing());
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.01);
    let before = ws.frames_rendered;
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.move_selection(Vec2::new(20.0, 0.0));
    run(&mut ws, &ctx, 0.1);
    assert!(
        ws.frames_rendered > before,
        "the edit shows up, not a stale picture"
    );
    assert!(
        ws.frame_cache
            .iter()
            .all(|c| c.revision == ws.active().revision()),
        "pictures of older document states are dropped"
    );
    // The limits hold however many different pictures come through.
    for _ in 0..(FRAME_CACHE_ENTRIES + 10) {
        ws.frame_cache.push(CachedFrame {
            context: ws.context_serial,
            revision: ws.active().revision(),
            signature: ws.frame_cache.len() as u64 + 1_000,
            view: ws.texture.as_ref().expect("texture").view,
            handle: ws.texture.as_ref().expect("texture").handle.clone(),
        });
    }
    ws.trim_frame_cache();
    assert!(ws.frame_cache.len() <= FRAME_CACHE_ENTRIES);
    assert!(!ws.frame_cache.is_empty());
}

#[test]
fn with_workers_a_frame_arrives_later_and_the_pass_never_waits_for_it() {
    let (ctx, mut ws) = (egui::Context::default(), held_then_changing());
    ws.start_background_threads(&ctx);
    run(&mut ws, &ctx, 0.0);
    // Nothing has been drawn yet unless the worker was already quick; either way the pass
    // returned, and a texture shows up on a later pass.
    wait_until(&mut ws, &ctx, |w| w.texture.is_some());
    wait_until(&mut ws, &ctx, |w| w.pending_render.is_none());
    let texture = ws.texture.as_ref().expect("texture");
    assert_eq!(texture.key.2, 0);
    assert!(ws.frames_rendered >= 1);
}

#[test]
fn with_workers_only_the_latest_request_is_shown() {
    let (ctx, mut ws) = (egui::Context::default(), held_then_changing());
    ws.start_background_threads(&ctx);
    run(&mut ws, &ctx, 0.0);
    // Ask for a different frame on every pass without waiting for any of them.
    for frame in [12, 3, 12, 20, 12] {
        ws.set_frame(frame);
        run(&mut ws, &ctx, 0.1);
    }
    wait_until(&mut ws, &ctx, |w| {
        w.pending_render.is_none() && w.texture.as_ref().is_some_and(|t| t.key.2 == 12)
    });
    assert_eq!(ws.texture.as_ref().map(|t| t.key.2), Some(12));
}

#[test]
fn with_workers_saving_does_not_block_and_edits_made_meanwhile_stay_unsaved() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.start_background_threads(&ctx);
    ws.commit_rectangle(Pos2::new(1.0, 1.0), Pos2::new(40.0, 40.0));
    ws.save_as(&path);
    assert!(
        ws.has_pending_io() || path.exists(),
        "the save was handed to a worker"
    );
    // An edit made right after, before the result is applied.
    ws.commit_rectangle(Pos2::new(50.0, 50.0), Pos2::new(90.0, 90.0));
    wait_until(&mut ws, &ctx, |w| !w.has_pending_io());
    assert_eq!(ws.path(), Some(path.as_path()));
    assert!(ws.is_dirty(), "the second rectangle is not in the file");
    let saved = flare_storage::load(&path).expect("load");
    assert_eq!(
        saved.scenes()[0].timeline().layers().len(),
        2,
        "only the first rectangle"
    );
    ws.undo();
    assert!(!ws.is_dirty(), "undoing back to what was written is clean");
}

#[test]
fn with_workers_opening_replaces_the_document_when_the_file_has_been_read() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    let mut source = workspace();
    source.commit_rectangle(Pos2::new(1.0, 1.0), Pos2::new(40.0, 40.0));
    source.save_as(&path);

    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.start_background_threads(&ctx);
    ws.open(&path);
    assert_eq!(
        layer_count(&ws),
        1,
        "not replaced until the file has been read"
    );
    wait_until(&mut ws, &ctx, |w| w.path() == Some(path.as_path()));
    assert_eq!(layer_count(&ws), 2);
    assert!(!ws.is_dirty());
    assert!(
        ws.background.mode() == Mode::Threads,
        "opening keeps the workers"
    );
}

#[test]
fn work_started_before_a_project_changed_does_not_replace_it() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    let mut source = workspace();
    source.save_as(&path);

    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.start_background_threads(&ctx);
    ws.open(&path);
    ws.commit_rectangle(Pos2::new(1.0, 1.0), Pos2::new(40.0, 40.0)); // edit while it opens
    wait_until(&mut ws, &ctx, |w| w.opening.is_none());
    assert_eq!(layer_count(&ws), 2, "the user's edit was kept");
    assert!(ws.status().is_some_and(|m| m.contains("changed while")));
}

#[test]
fn finishing_background_work_applies_pending_saves() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.start_background_threads(&ctx);
    ws.commit_rectangle(Pos2::new(1.0, 1.0), Pos2::new(40.0, 40.0));
    ws.save_as(&path);
    ws.finish_background();
    assert!(!ws.has_pending_io());
    assert!(!ws.is_dirty());
    assert!(path.exists());
}

#[test]
fn mask_layer_toggles_through_the_menu_and_changes_what_is_pickable() {
    let mut ws = two_rectangles();
    // Layers top to bottom: [Layer 1, first rectangle (x 10..50), second (x 100..140)].
    let layers = ws.document().scenes()[0].timeline().layers();
    let (mask_id, masked_id) = (layers[1].id(), layers[2].id());
    let at_mask = Pos2::new(30.0, 30.0);
    let at_masked = Pos2::new(120.0, 30.0);
    assert_eq!(ws.hit(at_mask).map(|s| s.layer), Some(mask_id));
    assert_eq!(ws.hit(at_masked).map(|s| s.layer), Some(masked_id));

    ws.select_layer(mask_id);
    assert_eq!(ws.label(MenuAction::ToggleLayerMask), "Mask Layer Below");
    ws.perform(MenuAction::ToggleLayerMask);
    assert!(ws.layer(mask_id).expect("layer").is_mask());
    assert_eq!(ws.label(MenuAction::ToggleLayerMask), "Remove Mask");
    assert!(
        ws.hit(at_mask).is_none(),
        "a mask is never drawn, so it cannot be picked"
    );
    assert!(
        ws.hit(at_masked).is_none(),
        "the masked art lies outside the mask, so nothing of it shows"
    );

    ws.undo();
    assert!(
        !ws.layer(mask_id).expect("layer").is_mask(),
        "one undo step"
    );
    assert_eq!(ws.hit(at_masked).map(|s| s.layer), Some(masked_id));
}

#[test]
fn selection_bounds_follow_the_selection_and_the_document_not_a_stale_cache() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    let first = ws.selection_rects();
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].min, Pos2::new(10.0, 10.0));
    assert_eq!(
        ws.selection_rects(),
        first,
        "asking again gives the same answer"
    );
    // A different span of the same count must not reuse the first one's bounds.
    ws.select_at(Pos2::new(120.0, 30.0), false);
    let second = ws.selection_rects();
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].min, Pos2::new(100.0, 10.0));
    // An edit that moves the selected art changes them too.
    ws.move_selection(Vec2::new(5.0, 0.0));
    assert_eq!(ws.selection_rects()[0].min, Pos2::new(105.0, 10.0));
    ws.undo();
    ws.select_at(Pos2::new(120.0, 30.0), false);
    assert_eq!(ws.selection_rects()[0].min, Pos2::new(100.0, 10.0));
    assert_eq!(
        ws.selection_bounds().map(|b| b.min),
        Some(Pos2::new(100.0, 10.0))
    );
}

#[test]
fn a_selection_made_stale_by_an_edit_is_dropped_on_the_next_frame_not_kept() {
    let (ctx, mut ws) = (egui::Context::default(), two_rectangles());
    ws.select_at(Pos2::new(30.0, 30.0), false);
    run(&mut ws, &ctx, 0.0);
    let layer = ws.selected.expect("selected").layer;
    ws.toggle_layer_locked(layer);
    run(&mut ws, &ctx, 0.1);
    assert!(
        !ws.has_selection(),
        "locking the layer dropped the selection"
    );
}

#[test]
fn dragging_outside_a_corner_rotates_the_selection_about_its_center() {
    let mut ws = wide_rectangle_selected();
    ws.set_tool(Tool::FreeTransform);
    ws.begin_drag(Pos2::new(120.0, 70.0)); // just outside the bottom-right corner
    assert_eq!(ws.drag.map(|d| d.mode), Some(DragMode::Rotate));
    // The pointer is 30.26 degrees clockwise of the center's right; turn it a quarter.
    ws.update_drag(Pos2::new(25.0, 95.0));
    ws.end_drag();
    let b = shape_bounds(&ws);
    assert!(
        (b.width() - 50.0).abs() < 0.01 && (b.height() - 100.0).abs() < 0.01,
        "{b:?}"
    );
    assert!(
        (b.center() - Pos2::new(60.0, 35.0)).length() < 0.01,
        "about its own center"
    );
    ws.undo();
    assert_eq!(shape_bounds(&ws).width(), 100.0, "one undo step");
}

#[test]
fn shift_snaps_a_rotation_and_a_cancelled_one_changes_nothing() {
    let mut ws = wide_rectangle_selected();
    ws.set_tool(Tool::FreeTransform);
    ws.uniform_scale = true;
    ws.begin_drag(Pos2::new(120.0, 70.0));
    // About 36 degrees: snapped to 30.
    let a = 30.26_f32.to_radians() + 36.0_f32.to_radians();
    let r = 69.5_f32;
    ws.update_drag(Pos2::new(60.0 + r * a.cos(), 35.0 + r * a.sin()));
    let drag = ws.drag.expect("drag");
    let matrix = ws.drag_matrix(&drag).expect("rotation");
    let angle = matrix.matrix2.x_axis.y.atan2(matrix.matrix2.x_axis.x);
    assert!((angle - 30.0_f64.to_radians()).abs() < 1e-6, "{angle}");
    ws.cancel_drag();
    ws.end_drag();
    assert_eq!(shape_bounds(&ws).width(), 100.0);
}

#[test]
fn alt_makes_an_edge_handle_skew_and_corners_still_scale() {
    let mut ws = wide_rectangle_selected();
    ws.set_tool(Tool::FreeTransform);
    ws.skew_modifier = true;
    ws.begin_drag(Pos2::new(110.0, 60.0)); // the bottom-right corner
    assert_eq!(
        ws.drag.map(|d| d.mode),
        Some(DragMode::Scale(Handle::BottomRight))
    );
    ws.cancel_drag();
    ws.end_drag();

    ws.begin_drag(Pos2::new(60.0, 10.0)); // the top edge
    assert_eq!(ws.drag.map(|d| d.mode), Some(DragMode::Skew(Handle::Top)));
    ws.update_drag(Pos2::new(85.0, 10.0));
    ws.end_drag();
    let shape = selected_shape(&ws);
    let first = shape.path().iter().find_map(|s| match s {
        flare_document::PathSegment::MoveTo(p) => Some((p.as_vec().x, p.as_vec().y)),
        _ => None,
    });
    assert_eq!(
        first,
        Some((35.0, 10.0)),
        "the top-left corner slid right by 25"
    );
    // The bottom edge stayed where it was.
    let bottom = shape.path().iter().filter_map(|s| match s {
        flare_document::PathSegment::LineTo(p) if p.as_vec().y == 60.0 => Some(p.as_vec().x),
        _ => None,
    });
    assert_eq!(bottom.collect::<Vec<_>>(), vec![110.0, 10.0]);
}

#[test]
fn a_tweened_instance_can_be_rotated_but_not_skewed() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    ws.convert_selection_to_symbol();
    ws.set_selection_tween(Tween::Linear);
    let scene = ws.scene_id().expect("scene");
    let at = ws.selected.expect("instance");
    let transform = |ws: &Workspace| match ops::find_span(ws.document(), scene, at)
        .expect("span")
        .content()
    {
        Content::Instance(i) => i.transform(),
        _ => panic!("an instance expected"),
    };
    let before = transform(&ws);
    let bounds = ws.selection_bounds().expect("bounds");
    ws.apply_to_selection(handles::skew_matrix(bounds, Handle::Top, 0.5));
    assert_eq!(transform(&ws), before, "refused");
    assert!(
        ws.status().is_some_and(|m| m.contains("cannot be skewed")),
        "{:?}",
        ws.status()
    );
    ws.apply_to_selection(handles::rotation_matrix(bounds.center(), 0.3));
    assert_ne!(transform(&ws), before, "a rotation is fine in a tween");
}

#[test]
fn copy_and_paste_frames_move_a_run_of_frames_to_the_playhead() {
    let (mut ws, layer) = two_exposures();
    assert_eq!(layer_spans(&ws, layer), vec![(0, 12), (12, 12)]);
    ws.select_frames(layer, 14, 10); // either order
    ws.select_layer(layer);
    ws.perform(MenuAction::CopyFrames);
    assert!(ws.is_enabled(MenuAction::PasteFrames));
    // Paste onto frames 20..25: the second exposure is cut and the pasted run fills in.
    ws.set_frame(20);
    ws.perform(MenuAction::PasteFrames);
    assert_eq!(
        layer_spans(&ws, layer),
        vec![(0, 12), (12, 8), (20, 2), (22, 3)],
        "frames 10..12 of the first exposure and 12..15 of the second, from frame 20"
    );
    let r = ws.frame_range.expect("pasted frames are selected");
    assert_eq!((r.from, r.to), (20, 24));
    ws.undo();
    assert_eq!(
        layer_spans(&ws, layer),
        vec![(0, 12), (12, 12)],
        "one undo step"
    );
}

#[test]
fn with_no_frames_selected_copy_takes_the_current_frame() {
    let (mut ws, layer) = two_exposures();
    ws.select_layer(layer);
    ws.set_frame(3);
    ws.perform(MenuAction::CopyFrames);
    ws.set_frame(23); // the last frame of the scene
    ws.perform(MenuAction::PasteFrames);
    assert_eq!(
        layer_spans(&ws, layer),
        vec![(0, 12), (12, 11), (23, 1)],
        "the one copied frame replaced the last frame"
    );
}
