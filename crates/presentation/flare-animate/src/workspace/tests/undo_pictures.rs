//! Undo and redo show a picture that was drawn before without drawing it again.

use super::*;

fn settle(ws: &mut Workspace, ctx: &egui::Context, time: f64) {
    for pass in 0..3 {
        run(ws, ctx, time + f64::from(pass) * 0.01);
    }
}

fn shown_revision(ws: &Workspace) -> Option<u64> {
    ws.texture.as_ref().map(|t| t.key.1)
}

#[test]
fn undo_and_redo_reuse_the_pictures_already_drawn() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(40.0, 40.0));
    settle(&mut ws, &ctx, 0.0);
    let one_rectangle = ws.frames_rendered;
    ws.commit_rectangle(Pos2::new(60.0, 0.0), Pos2::new(100.0, 40.0));
    settle(&mut ws, &ctx, 0.1);
    let two_rectangles = ws.frames_rendered;
    assert!(two_rectangles > one_rectangle, "a new state is drawn");

    ws.undo();
    settle(&mut ws, &ctx, 0.2);
    assert_eq!(
        ws.frames_rendered, two_rectangles,
        "undo brings back a picture already drawn, so nothing is drawn"
    );
    assert_eq!(
        shown_revision(&ws),
        Some(ws.active().revision()),
        "and it is the picture of the state now current"
    );

    ws.redo();
    settle(&mut ws, &ctx, 0.3);
    assert_eq!(ws.frames_rendered, two_rectangles, "so does redo");
    assert_eq!(shown_revision(&ws), Some(ws.active().revision()));
}

#[test]
fn a_new_edit_after_undo_is_drawn_not_taken_from_an_older_picture() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(40.0, 40.0));
    settle(&mut ws, &ctx, 0.0);
    ws.commit_rectangle(Pos2::new(60.0, 0.0), Pos2::new(100.0, 40.0));
    settle(&mut ws, &ctx, 0.1);
    ws.undo();
    settle(&mut ws, &ctx, 0.2);
    let before = ws.frames_rendered;
    // A different second rectangle: the state is new, whatever the old pictures show.
    ws.commit_rectangle(Pos2::new(120.0, 0.0), Pos2::new(160.0, 40.0));
    settle(&mut ws, &ctx, 0.3);
    assert!(ws.frames_rendered > before, "the new state is drawn");
    assert_eq!(shown_revision(&ws), Some(ws.active().revision()));
}
