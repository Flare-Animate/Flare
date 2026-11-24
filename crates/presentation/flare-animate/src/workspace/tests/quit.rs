//! Quitting asks about unsaved work in the window, one document at a time, and never waits.

use super::*;
use crate::workspace::close_prompt::Choice;

fn drawn() -> Workspace {
    let mut ws = workspace();
    rect(&mut ws);
    ws
}

#[test]
fn quitting_with_nothing_unsaved_closes_at_once() {
    let mut ws = workspace();
    ws.request_quit();
    assert!(ws.take_quit_request());
    assert!(!ws.take_quit_request(), "a quit is agreed to once");
    assert_eq!(ws.close_prompt, None, "nothing was asked");
}

#[test]
fn unsaved_work_is_asked_about_and_cancel_keeps_the_app_open() {
    let (ctx, mut ws) = (egui::Context::default(), drawn());
    ws.request_quit();
    assert!(!ws.take_quit_request(), "the drawing is not saved");
    run(&mut ws, &ctx, 0.0);
    assert_eq!(ws.close_prompt, Some(0), "the question is in the window");
    assert!(!ws.take_quit_request(), "the question is not answered yet");
    ws.apply_close_choice(0, Choice::Keep);
    run(&mut ws, &ctx, 0.1);
    assert_eq!(
        ws.close_prompt, None,
        "Cancel ended the quit, not asked again"
    );
    assert!(!ws.take_quit_request());
    assert_eq!(drawing_count(&ws), 1, "the drawing is still there");
}

#[test]
fn dont_save_on_each_document_in_turn_lets_the_app_close() {
    let (ctx, mut ws) = (egui::Context::default(), drawn());
    ws.new_tab();
    rect(&mut ws);
    assert_eq!(ws.position, 1);
    ws.request_quit();
    run(&mut ws, &ctx, 0.0);
    assert_eq!(ws.close_prompt, Some(0), "the first unsaved document first");
    assert_eq!(ws.position, 0, "brought forward to be seen");
    ws.apply_close_choice(0, Choice::Discard);
    assert!(!ws.take_quit_request(), "the other one is still unsaved");
    run(&mut ws, &ctx, 0.1);
    assert_eq!(ws.close_prompt, Some(0), "then the next");
    ws.apply_close_choice(0, Choice::Discard);
    assert!(ws.take_quit_request());
    assert!(!ws.any_dirty());
}

#[test]
fn saving_an_untitled_document_waits_for_the_picker_then_quits() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("kept.flare");
    let (ctx, mut ws) = (egui::Context::default(), drawn());
    ws.request_quit();
    run(&mut ws, &ctx, 0.0);
    ws.apply_close_choice(0, Choice::Save);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::SaveAs));
    run(&mut ws, &ctx, 0.1);
    assert!(!ws.take_quit_request(), "the picker is still open");
    assert_eq!(ws.close_prompt, None, "and nothing else is asked meanwhile");
    ws.save_as(&path);
    ws.end_save_as();
    ws.settle_inline();
    run(&mut ws, &ctx, 0.2);
    assert!(path.exists(), "the project was written");
    assert!(ws.take_quit_request(), "and the app may close");
}

#[test]
fn a_save_as_cancelled_while_quitting_ends_the_quit() {
    let (ctx, mut ws) = (egui::Context::default(), drawn());
    ws.request_quit();
    run(&mut ws, &ctx, 0.0);
    ws.apply_close_choice(0, Choice::Save);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::SaveAs));
    ws.end_save_as();
    run(&mut ws, &ctx, 0.1);
    run(&mut ws, &ctx, 0.2);
    assert_eq!(ws.close_prompt, None, "not asked again in a loop");
    assert!(!ws.take_quit_request());
    assert!(ws.is_dirty(), "the work is kept");
}

#[test]
fn the_menu_quit_goes_through_the_same_questions() {
    let (ctx, mut ws) = (egui::Context::default(), drawn());
    ws.perform(MenuAction::Quit);
    assert!(!ws.take_quit_request());
    run(&mut ws, &ctx, 0.0);
    assert_eq!(ws.close_prompt, Some(0));
}
