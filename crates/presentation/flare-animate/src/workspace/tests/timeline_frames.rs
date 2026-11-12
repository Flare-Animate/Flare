use super::*;

#[test]
fn a_short_scene_gets_small_frames_not_the_width_of_the_panel() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    // A new document has one frame; its cell is a few points wide, as Animate's is.
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let width = ws.timeline_frame_width();
    assert!(
        (4.0..=24.0).contains(&width),
        "a frame is {width} points wide"
    );
}

#[test]
fn zoom_widens_and_narrows_the_frames_and_fit_fills_the_panel_with_the_scene() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    let normal = ws.timeline_frame_width();
    ws.zoom_timeline(timeline_view::STEP);
    assert!(ws.timeline_frame_width() > normal);
    ws.zoom_timeline(0.1);
    assert!(ws.timeline_frame_width() < normal);
    // Fit widens the cells until the scene's frames fill the panel, or as far as the range
    // allows, never past it: either way no more than a cell or two of empty space is left.
    ws.timeline_fit = true;
    run(&mut ws, &ctx, 0.1);
    assert!(!ws.timeline_fit, "it is done once");
    assert!(ws.timeline_zoom > normal / timeline_view::BASE_FRAME_WIDTH);
    let layout = ws.timeline_layout.expect("drawn");
    let cells = (layout.rect.width() - timeline::LABEL_W) / layout.per_frame;
    let frames = ws.scene_frames() as f32;
    assert!(
        ws.timeline_zoom == timeline_view::MAX_ZOOM || cells <= frames + 2.0,
        "{cells} cells drawn for {frames} frames"
    );
    assert!(!ws.is_dirty(), "the view is not the document");
}
